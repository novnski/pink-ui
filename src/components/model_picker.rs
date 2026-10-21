//! Model Picker – a chat composer's model menu with a live spec sheet.
//!
//! An original in luna's design language. A quiet trigger names the current
//! model; its menu opens above the composer with the models on the left and
//! a spec sheet on the right. Point at a model (or arrow to it) and its
//! speed, depth and cost meters glide to the new values while the latency,
//! per-reply price and context figures roll over. "Think longer" unfolds a
//! three-step reasoning budget whose thumb glides, and the figures follow it.
//!
//! Keys: ↑/↓ point, Enter chooses, Space toggles reasoning, ←/→ change the
//! budget, Escape closes.
//!
//! ```ignore
//! ModelPicker::new("model", default_models())
//!     .selected("helix-pro")
//!     .on_change(|model, _, _| println!("now using {}", model.name))
//! ```

use std::rc::Rc;

use gpui::{
    AnimationExt, AnyView, App, Bounds, ElementId, FocusHandle, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseDownEvent, Pixels, SharedString, Stateful, Transformation, Window, canvas,
    deferred, div, hsla, percentage, prelude::*, px, relative, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, text},
    motion::{self, EASE, EASE_OUT_QUINT, GLIDE, MENU_IN, MotionSpec},
    primitives::shadow,
    story,
};

pub const NAME: &str = "Model Picker";
pub const DESCRIPTION: &str = "A composer's model menu with a live spec sheet – point at a model to glide its meters and roll its latency and price, then unfold a reasoning budget";

const ICON_AUTO: &str = "icons/model-picker/auto.svg";
const ICON_ULTRA: &str = "icons/model-picker/ultra.svg";
const ICON_PRO: &str = "icons/model-picker/pro.svg";
const ICON_FLASH: &str = "icons/model-picker/flash.svg";
const ICON_CHEVRON: &str = "icons/model-picker/chevron.svg";
const ICON_CHECK: &str = "icons/model-picker/check.svg";
const ICON_HOURGLASS: &str = "icons/model-picker/hourglass.svg";
const ICON_SEND: &str = "icons/model-picker/send.svg";
const ICON_PLUS: &str = "icons/model-picker/plus.svg";
const ICON_MIC: &str = "icons/model-picker/mic.svg";

const MENU_W: f32 = 548.;
const LIST_W: f32 = 236.;
const ROW_H: f32 = 44.;
const TRIGGER_H: f32 = 28.;
const MENU_GAP: f32 = 10.;
const SEG_W: f32 = 76.;
/// Tokens an average reply spends on the answer itself.
const REPLY_TOKENS: f32 = 700.;

type ChangeHandler = Rc<dyn Fn(&ModelOption, &mut Window, &mut App)>;
type ReasoningHandler = Rc<dyn Fn(Option<Budget>, &mut Window, &mut App)>;

/// How long the model may think before it answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    Light,
    Balanced,
    Deep,
}

impl Budget {
    pub const ALL: [Budget; 3] = [Budget::Light, Budget::Balanced, Budget::Deep];

    pub fn label(self) -> &'static str {
        match self {
            Budget::Light => "Light",
            Budget::Balanced => "Balanced",
            Budget::Deep => "Deep",
        }
    }

    fn index(self) -> usize {
        match self {
            Budget::Light => 0,
            Budget::Balanced => 1,
            Budget::Deep => 2,
        }
    }

    /// Thinking tokens spent per reply.
    fn tokens(self) -> f32 {
        match self {
            Budget::Light => 2_000.,
            Budget::Balanced => 8_000.,
            Budget::Deep => 24_000.,
        }
    }

    /// How much longer the first token takes.
    fn wait(self) -> f32 {
        match self {
            Budget::Light => 2.2,
            Budget::Balanced => 4.0,
            Budget::Deep => 7.5,
        }
    }

    fn note(self) -> &'static str {
        match self {
            Budget::Light => "Up to 2K thinking tokens",
            Budget::Balanced => "Up to 8K thinking tokens",
            Budget::Deep => "Up to 24K thinking tokens",
        }
    }
}

/// One model in the menu. Meter values run from 0 to 1; a routed model
/// (like Auto) gives a range, drawn as a solid run plus a lighter reach.
#[derive(Clone, Debug)]
pub struct ModelOption {
    pub id: SharedString,
    pub name: SharedString,
    /// A few words under the name in the list.
    pub summary: SharedString,
    /// A sentence for the spec sheet.
    pub about: SharedString,
    /// An SVG path under `assets/`, drawn in the model's tile.
    pub icon: Option<SharedString>,
    /// Small pill after the name, e.g. "New".
    pub tag: Option<SharedString>,
    pub speed: (f32, f32),
    pub depth: (f32, f32),
    pub cost: (f32, f32),
    /// Seconds to the first token, without reasoning.
    pub first_token: (f32, f32),
    /// USD per million output tokens.
    pub price: (f32, f32),
    /// Context window, e.g. "400K".
    pub context: SharedString,
    /// Whether extended reasoning is available.
    pub reasoning: bool,
}

impl ModelOption {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            summary: SharedString::default(),
            about: SharedString::default(),
            icon: None,
            tag: None,
            speed: (0.5, 0.5),
            depth: (0.5, 0.5),
            cost: (0.5, 0.5),
            first_token: (1.0, 1.0),
            price: (10.0, 10.0),
            context: "128K".into(),
            reasoning: true,
        }
    }

    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn about(mut self, about: impl Into<SharedString>) -> Self {
        self.about = about.into();
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn tag(mut self, tag: impl Into<SharedString>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Speed, depth and cost, each from 0 to 1.
    pub fn meters(mut self, speed: f32, depth: f32, cost: f32) -> Self {
        self.speed = (speed, speed);
        self.depth = (depth, depth);
        self.cost = (cost, cost);
        self
    }

    /// Meters for a routed model: each one a `(low, high)` range.
    pub fn meter_ranges(mut self, speed: (f32, f32), depth: (f32, f32), cost: (f32, f32)) -> Self {
        self.speed = speed;
        self.depth = depth;
        self.cost = cost;
        self
    }

    /// Seconds to the first token (pass the same value twice for a fixed model).
    pub fn first_token(mut self, low: f32, high: f32) -> Self {
        self.first_token = (low, high);
        self
    }

    /// USD per million output tokens (pass the same value twice for a fixed model).
    pub fn price(mut self, low: f32, high: f32) -> Self {
        self.price = (low, high);
        self
    }

    pub fn context(mut self, context: impl Into<SharedString>) -> Self {
        self.context = context.into();
        self
    }

    /// Hides extended reasoning for this model.
    pub fn no_reasoning(mut self) -> Self {
        self.reasoning = false;
        self
    }

    fn first_token_label(&self, budget: Option<Budget>) -> String {
        let extra = budget.map_or(0., Budget::wait);
        let (lo, hi) = (self.first_token.0 + extra, self.first_token.1 + extra);
        if (hi - lo).abs() < 0.05 {
            format!("{} s", seconds(lo))
        } else {
            format!("{}–{} s", seconds(lo), seconds(hi))
        }
    }

    fn reply_cost_label(&self, budget: Option<Budget>) -> String {
        let tokens = REPLY_TOKENS + budget.map_or(0., Budget::tokens);
        let (lo, hi) = (self.price.0 * tokens / 1e6, self.price.1 * tokens / 1e6);
        if (hi - lo).abs() < 0.0005 {
            format!("${}", cents(lo))
        } else {
            // A routed model only promises a ceiling.
            format!("≤ ${}", cents(hi))
        }
    }
}

fn seconds(value: f32) -> String {
    if value < 10. {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    }
}

fn cents(value: f32) -> String {
    if value < 0.01 {
        format!("{value:.4}")
    } else {
        format!("{value:.3}")
    }
}

/// Auto, Helix Ultra, Helix Pro and Helix Flash.
pub fn default_models() -> Vec<ModelOption> {
    vec![
        ModelOption::new("auto", "Auto")
            .summary("Picks per message")
            .about("Sends each message to the model that fits it")
            .icon(ICON_AUTO)
            .meter_ranges((0.45, 0.95), (0.4, 0.95), (0.1, 0.8))
            .first_token(0.3, 1.6)
            .price(1.2, 30.)
            .context("200K"),
        ModelOption::new("helix-ultra", "Helix Ultra")
            .summary("Hard, long problems")
            .about("Deep analysis across long documents and code")
            .icon(ICON_ULTRA)
            .tag("New")
            .meters(0.38, 0.97, 0.88)
            .first_token(1.6, 1.6)
            .price(30., 30.)
            .context("1M"),
        ModelOption::new("helix-pro", "Helix Pro")
            .summary("Everyday work")
            .about("Balanced speed and depth for most tasks")
            .icon(ICON_PRO)
            .meters(0.7, 0.78, 0.42)
            .first_token(0.8, 0.8)
            .price(9., 9.)
            .context("400K"),
        ModelOption::new("helix-flash", "Helix Flash")
            .summary("Quick and light")
            .about("Instant replies for drafts, lookups and chat")
            .icon(ICON_FLASH)
            .meters(0.96, 0.48, 0.1)
            .first_token(0.3, 0.3)
            .price(1.2, 1.2)
            .context("200K")
            .no_reasoning(),
    ]
}

/// Which side of the trigger the menu opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Above,
    Below,
}

#[derive(Clone)]
struct PickerState {
    focus: FocusHandle,
    open: bool,
    /// Bumped on every open so the entrance and the meter fill replay.
    opened: u32,
    selected: usize,
    pointed: usize,
    reasoning: bool,
    budget: Budget,
    trigger: Bounds<Pixels>,
}

impl PickerState {
    fn set_open(&mut self, open: bool) {
        if open && !self.open {
            self.opened += 1;
            self.pointed = self.selected;
        }
        self.open = open;
    }
}

#[derive(IntoElement)]
pub struct ModelPicker {
    id: ElementId,
    models: Vec<ModelOption>,
    selected: Option<SharedString>,
    default_open: bool,
    autofocus: bool,
    reasoning: bool,
    budget: Budget,
    placement: Placement,
    on_change: Option<ChangeHandler>,
    on_reasoning_change: Option<ReasoningHandler>,
}

impl ModelPicker {
    pub fn new(id: impl Into<ElementId>, models: Vec<ModelOption>) -> Self {
        Self {
            id: id.into(),
            models,
            selected: None,
            default_open: false,
            autofocus: false,
            reasoning: false,
            budget: Budget::Balanced,
            placement: Placement::Above,
            on_change: None,
            on_reasoning_change: None,
        }
    }

    /// The model chosen at first (by id); defaults to the first one.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
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

    /// Starts with extended reasoning on, at `budget`.
    pub fn reasoning(mut self, on: bool, budget: Budget) -> Self {
        self.reasoning = on;
        self.budget = budget;
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

    /// Called with the budget when reasoning turns on or changes, `None` when it turns off.
    pub fn on_reasoning_change(
        mut self,
        handler: impl Fn(Option<Budget>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reasoning_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ModelPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let models = Rc::new(self.models);
        let count = models.len();
        let initial = self
            .selected
            .as_ref()
            .and_then(|id| models.iter().position(|m| &m.id == id))
            .unwrap_or(0);
        let (default_open, autofocus) = (self.default_open, self.autofocus);
        let (reasoning, budget) = (self.reasoning, self.budget);

        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| {
                let focus = cx.focus_handle();
                if autofocus {
                    let handle = focus.clone();
                    window.defer(cx, move |window, _| window.focus(&handle));
                }
                PickerState {
                    focus,
                    open: default_open,
                    opened: u32::from(default_open),
                    selected: initial,
                    pointed: initial,
                    reasoning,
                    budget,
                    trigger: Bounds::default(),
                }
            },
        );
        let s = state.read(cx).clone();
        if count == 0 {
            return div().id(self.id);
        }
        let selected = s.selected.min(count - 1);
        let current = &models[selected];
        let reasoning_on = s.reasoning && current.reasoning;

        let on_change = self.on_change.clone();
        let on_reasoning = self.on_reasoning_change.clone();

        // Keyboard: the trigger and the open menu share one focus handle.
        let on_key = {
            let state = state.clone();
            let models = models.clone();
            let on_change = on_change.clone();
            let on_reasoning = on_reasoning.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let mut chosen = None;
                let mut reasoning_event = None;
                let handled = state.update(cx, |s, cx| {
                    let pointed_reasons = models[s.pointed.min(count - 1)].reasoning;
                    let handled = match (s.open, event.keystroke.key.as_str()) {
                        (false, "enter" | "space" | "down" | "up") => {
                            s.set_open(true);
                            true
                        }
                        (true, "escape") => {
                            s.set_open(false);
                            true
                        }
                        (true, "down") => {
                            s.pointed = (s.pointed + 1) % count;
                            true
                        }
                        (true, "up") => {
                            s.pointed = (s.pointed + count - 1) % count;
                            true
                        }
                        (true, "home") => {
                            s.pointed = 0;
                            true
                        }
                        (true, "end") => {
                            s.pointed = count - 1;
                            true
                        }
                        (true, "enter") => {
                            s.selected = s.pointed;
                            s.set_open(false);
                            chosen = Some(s.selected);
                            true
                        }
                        (true, "space") if pointed_reasons => {
                            s.reasoning = !s.reasoning;
                            reasoning_event = Some(s.reasoning.then_some(s.budget));
                            true
                        }
                        (true, "left" | "right") if pointed_reasons => {
                            let ix = s.budget.index();
                            let ix = if event.keystroke.key == "left" {
                                ix.saturating_sub(1)
                            } else {
                                (ix + 1).min(2)
                            };
                            s.budget = Budget::ALL[ix];
                            s.reasoning = true;
                            reasoning_event = Some(Some(s.budget));
                            true
                        }
                        _ => false,
                    };
                    if handled {
                        cx.notify();
                    }
                    handled
                });
                if handled {
                    cx.stop_propagation();
                }
                if let (Some(ix), Some(f)) = (chosen, on_change.as_ref()) {
                    f(&models[ix], window, cx);
                }
                if let (Some(value), Some(f)) = (reasoning_event, on_reasoning.as_ref()) {
                    f(value, window, cx);
                }
            }
        };

        let trigger = trigger(&key, current, &s, reasoning_on, &t, &state);
        let menu = s.open.then(|| {
            let content = menu(&key, &models, &s, &t, &state, on_change, on_reasoning);
            let opened = s.opened;
            let placement = self.placement;
            let closer = state.clone();
            let menu = div()
                .id("menu")
                .absolute()
                .left(px(-6.))
                .w(px(MENU_W))
                .occlude()
                .on_mouse_down_out(move |event: &MouseDownEvent, _, cx| {
                    closer.update(cx, |s, cx| {
                        if s.open && !s.trigger.contains(&event.position) {
                            s.set_open(false);
                            cx.notify();
                        }
                    })
                })
                .child(content)
                .with_animation(
                    ElementId::NamedInteger("menu-in".into(), u64::from(opened)),
                    MENU_IN.with_duration(240).animation(),
                    move |el, k| {
                        let offset = px(TRIGGER_H + MENU_GAP - 6. * (1. - k));
                        let el = el.opacity(k);
                        match placement {
                            Placement::Above => el.bottom(offset),
                            Placement::Below => el.top(offset),
                        }
                    },
                );
            deferred(menu).with_priority(1)
        });

        motion::keep_alive(window);
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .font_family(luna::SANS)
            .track_focus(&s.focus)
            .on_key_down(on_key)
            .child(trigger)
            .children(menu)
    }
}

fn trigger(
    key: &str,
    current: &ModelOption,
    s: &PickerState,
    reasoning_on: bool,
    t: &Luna,
    state: &gpui::Entity<PickerState>,
) -> Stateful<gpui::Div> {
    let turn = motion::follow(
        &format!("{key}-chevron"),
        if s.open { 0.5 } else { 0. },
        MotionSpec::new(280, EASE_OUT_QUINT),
    );
    let rest = if s.open {
        t.fill(FillTier::Secondary)
    } else {
        t.fill(FillTier::Tertiary).opacity(0.)
    };
    let hover = if s.open {
        t.fill(FillTier::Secondary)
    } else {
        t.fill(FillTier::Tertiary)
    };
    let toggler = state.clone();
    let measurer = state.clone();
    div()
        .id("trigger")
        .relative()
        .h(px(TRIGGER_H))
        .pl(px(8.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(radius::MD))
        .cursor_pointer()
        .text_size(px(text::BODY))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text)
        .hover_fill(format!("{key}-trigger"), rest, hover)
        .on_click(move |_, window, cx| {
            toggler.update(cx, |s, cx| {
                let open = !s.open;
                s.set_open(open);
                window.focus(&s.focus);
                cx.notify();
            })
        })
        .when_some(current.icon.clone(), |el, icon| {
            el.child(svg().path(icon).size(px(14.)).text_color(t.text_muted))
        })
        .child(current.name.clone())
        .when(reasoning_on, |el| {
            el.child(
                div()
                    .h(px(18.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .rounded(px(radius::XS))
                    .bg(t.accent.opacity(if t.dark { 0.18 } else { 0.1 }))
                    .text_size(px(text::CAPTION))
                    .text_color(t.accent)
                    .child(
                        svg()
                            .path(ICON_HOURGLASS)
                            .size(px(11.))
                            .text_color(t.accent),
                    )
                    .child(s.budget.label()),
            )
        })
        .child(
            svg()
                .path(ICON_CHEVRON)
                .size(px(14.))
                .text_color(t.text_faint)
                .with_transformation(Transformation::rotate(percentage(turn))),
        )
        .child(
            canvas(
                move |bounds, _, cx| measurer.update(cx, |s, _| s.trigger = bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
}

fn menu(
    key: &str,
    models: &Rc<Vec<ModelOption>>,
    s: &PickerState,
    t: &Luna,
    state: &gpui::Entity<PickerState>,
    on_change: Option<ChangeHandler>,
    on_reasoning: Option<ReasoningHandler>,
) -> impl IntoElement {
    let count = models.len();
    let pointed = s.pointed.min(count - 1);
    let plate_top = motion::follow(
        &format!("{key}-plate"),
        pointed as f32 * ROW_H,
        MotionSpec::new(220, EASE_OUT_QUINT),
    );

    let rows = models.iter().enumerate().map(|(ix, m)| {
        let lit = motion::follow(
            &format!("{key}-lit-{ix}"),
            if ix == pointed { 1. } else { 0. },
            MotionSpec::new(160, EASE),
        );
        let pointer = state.clone();
        let chooser = state.clone();
        let models = models.clone();
        let on_change = on_change.clone();
        div()
            .id(("model", ix))
            .h(px(ROW_H))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .cursor_pointer()
            .on_mouse_move(move |_, _, cx| {
                pointer.update(cx, |s, cx| {
                    if s.pointed != ix {
                        s.pointed = ix;
                        cx.notify();
                    }
                })
            })
            .on_click(move |_, window, cx| {
                chooser.update(cx, |s, cx| {
                    s.selected = ix;
                    s.set_open(false);
                    cx.notify();
                });
                if let Some(f) = on_change.as_ref() {
                    f(&models[ix], window, cx);
                }
            })
            .child(
                div()
                    .size(px(28.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::MD))
                    .border_1()
                    .border_color(motion::mix(t.border, t.border_strong, lit))
                    .bg(motion::mix(
                        t.fill(FillTier::Quaternary),
                        t.surface_overlay,
                        lit,
                    ))
                    .when_some(m.icon.clone(), |el, icon| {
                        el.child(svg().path(icon).size(px(16.)).text_color(motion::mix(
                            t.text_faint,
                            t.text,
                            lit,
                        )))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(m.name.clone())
                            .when_some(m.tag.clone(), |el, tag| {
                                el.child(luna::badge(tag, Tone::Accent, t).h(px(16.)).px(px(5.)))
                            }),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(t.text_faint)
                            .child(m.summary.clone()),
                    ),
            )
            .when(ix == s.selected, |el| {
                el.child(
                    svg()
                        .path(ICON_CHECK)
                        .size(px(16.))
                        .flex_none()
                        .text_color(t.accent),
                )
            })
    });

    let list = div()
        .w(px(LIST_W))
        .flex_none()
        .p(px(6.))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(26.))
                .px(px(8.))
                .flex()
                .items_center()
                .text_size(px(text::FOOTNOTE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_faint)
                .child("Models"),
        )
        .child(
            div()
                .relative()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(plate_top))
                        .h(px(ROW_H))
                        .rounded(px(radius::MD))
                        .bg(t.fill(FillTier::Tertiary)),
                )
                .children(rows),
        );

    let model = &models[pointed];
    let budget = (s.reasoning && model.reasoning).then_some(s.budget);

    div()
        .w_full()
        .bg(t.surface_overlay)
        .border_1()
        .border_color(t.border)
        .rounded(px(radius::LG))
        .shadow(t.menu_shadow())
        .overflow_hidden()
        .text_color(t.text)
        .child(
            div()
                .flex()
                .child(list)
                .child(div().w(px(1.)).bg(t.border))
                .child(spec_sheet(key, model, budget, s.opened, t)),
        )
        .child(luna::divider(t))
        .child(reasoning_panel(key, model, s, t, state, on_reasoning))
}

/// The pointed model's name, meters and figures.
fn spec_sheet(
    key: &str,
    model: &ModelOption,
    budget: Option<Budget>,
    opened: u32,
    t: &Luna,
) -> impl IntoElement {
    let header = div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(text::CALLOUT))
                        .line_height(px(20.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(model.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(text::CAPTION))
                        .font_family(luna::MONO)
                        .text_color(t.text_faint)
                        .child(price_label(model)),
                ),
        )
        .child(
            div()
                .text_size(px(text::FOOTNOTE))
                .line_height(px(17.))
                .text_color(t.text_muted)
                .child(model.about.clone()),
        )
        .with_animation(
            SharedString::from(format!("{key}-spec-{}", model.id)),
            MotionSpec::new(260, EASE_OUT_QUINT).animation(),
            |el, k| el.relative().opacity(k).left(px(4. * (1. - k))),
        );

    let meters = div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(meter(
            key,
            "Speed",
            model.speed,
            speed_word(model.speed),
            opened,
            t,
        ))
        .child(meter(
            key,
            "Depth",
            model.depth,
            depth_word(model.depth),
            opened,
            t,
        ))
        .child(meter(
            key,
            "Cost",
            model.cost,
            cost_word(model.cost),
            opened,
            t,
        ));

    let figures = div()
        .flex()
        .gap(px(6.))
        .child(figure(
            key,
            "first",
            model.first_token_label(budget),
            "First token",
            t,
        ))
        .child(figure(
            key,
            "reply",
            model.reply_cost_label(budget),
            "Per reply",
            t,
        ))
        .child(figure(
            key,
            "context",
            model.context.to_string(),
            "Context",
            t,
        ));

    div()
        .flex_1()
        .min_w_0()
        .p(px(14.))
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(header)
        .child(meters)
        .child(figures)
}

fn price_label(model: &ModelOption) -> String {
    let (lo, hi) = model.price;
    if (hi - lo).abs() < 0.01 {
        format!("${lo:.0}/M")
    } else {
        format!("${lo:.0}–{hi:.0}/M")
    }
}

fn speed_word(range: (f32, f32)) -> &'static str {
    if range.1 - range.0 > 0.05 {
        return "Adapts";
    }
    match range.0 {
        v if v > 0.9 => "Instant",
        v if v > 0.6 => "Quick",
        v if v > 0.4 => "Steady",
        _ => "Patient",
    }
}

fn depth_word(range: (f32, f32)) -> &'static str {
    if range.1 - range.0 > 0.05 {
        return "Adapts";
    }
    match range.0 {
        v if v > 0.9 => "Deepest",
        v if v > 0.7 => "Strong",
        v if v > 0.4 => "Solid",
        _ => "Light",
    }
}

fn cost_word(range: (f32, f32)) -> &'static str {
    if range.1 - range.0 > 0.05 {
        return "Varies";
    }
    match range.0 {
        v if v > 0.75 => "$$$$",
        v if v > 0.5 => "$$$",
        v if v > 0.25 => "$$",
        _ => "$",
    }
}

/// A labelled bar that glides to its value; a range adds a lighter reach.
fn meter(
    key: &str,
    label: &'static str,
    (lo, hi): (f32, f32),
    word: &'static str,
    opened: u32,
    t: &Luna,
) -> impl IntoElement {
    // Keyed by the open count, so the bars fill from empty each time the menu opens.
    let low = motion::follow_from(&format!("{key}-{label}-lo-{opened}"), 0., lo, GLIDE);
    let high = motion::follow_from(&format!("{key}-{label}-hi-{opened}"), 0., hi, GLIDE);
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(16.))
        .child(
            div()
                .w(px(44.))
                .flex_none()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_muted)
                .child(label),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .h(px(6.))
                .rounded_full()
                .overflow_hidden()
                .bg(t.fill(FillTier::Secondary))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .h_full()
                        .w(relative(high.clamp(0., 1.)))
                        .rounded_full()
                        .bg(t.accent.opacity(0.3)),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .h_full()
                        .w(relative(low.clamp(0., 1.)))
                        .rounded_full()
                        .bg(t.accent),
                ),
        )
        .child(
            div()
                .w(px(46.))
                .flex_none()
                .flex()
                .justify_end()
                .overflow_hidden()
                .text_size(px(text::CAPTION))
                .text_color(t.text_faint)
                .child(roll(format!("{key}-{label}-word"), word.to_string(), 6.)),
        )
}

/// Text that rolls up into place whenever its value changes.
fn roll(id: String, value: String, rise: f32) -> impl IntoElement {
    div().child(value.clone()).with_animation(
        SharedString::from(format!("{id}-{value}")),
        MotionSpec::new(300, EASE_OUT_QUINT).animation(),
        move |el, k| el.relative().top(px(rise * (1. - k))).opacity(k),
    )
}

fn figure(key: &str, name: &str, value: String, label: &'static str, t: &Luna) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .px(px(9.))
        .py(px(7.))
        .rounded(px(radius::MD))
        .bg(t.fill(FillTier::Quaternary))
        .border_1()
        .border_color(t.border)
        .flex()
        .flex_col()
        .gap(px(1.))
        .child(
            div()
                .h(px(18.))
                .overflow_hidden()
                .text_size(px(text::BODY))
                .line_height(px(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .whitespace_nowrap()
                .child(roll(format!("{key}-fig-{name}"), value, 10.)),
        )
        .child(
            div()
                .text_size(px(text::CAPTION))
                .text_color(t.text_faint)
                .child(label),
        )
}

/// "Think longer" with its switch, and the budget that unfolds under it.
fn reasoning_panel(
    key: &str,
    model: &ModelOption,
    s: &PickerState,
    t: &Luna,
    state: &gpui::Entity<PickerState>,
    on_reasoning: Option<ReasoningHandler>,
) -> impl IntoElement {
    // The budget stays unfolded while reasoning is on, and dims on a model
    // without it, so the menu keeps its height while you browse.
    let available = model.reasoning;
    let on = s.reasoning;
    let unfold = motion::follow(&format!("{key}-unfold"), if on { 1. } else { 0. }, GLIDE);
    let lit = motion::follow(
        &format!("{key}-available"),
        if available { unfold } else { 0. },
        MotionSpec::new(200, EASE),
    );
    let note: SharedString = if available {
        "Works through the problem before it answers".into()
    } else {
        format!("Not available on {}", model.name).into()
    };

    let toggler = state.clone();
    let on_toggle = on_reasoning.clone();
    let toggle = switch(&format!("{key}-switch"), on, t)
        .when(!available, |el| el.opacity(0.45))
        .when(available, |el| {
            el.cursor_pointer().on_click(move |_, window, cx| {
                let value = toggler.update(cx, |s, cx| {
                    s.reasoning = !s.reasoning;
                    cx.notify();
                    s.reasoning.then_some(s.budget)
                });
                if let Some(f) = on_toggle.as_ref() {
                    f(value, window, cx);
                }
            })
        });

    let thumb_x = motion::follow(
        &format!("{key}-thumb"),
        s.budget.index() as f32 * SEG_W,
        MotionSpec::new(280, EASE_OUT_QUINT),
    );
    let thumb = if t.dark {
        t.surface_raised
    } else {
        t.surface_overlay
    };
    let segments = Budget::ALL.iter().map(|&b| {
        let picker = state.clone();
        let on_pick = on_reasoning.clone();
        let active = b == s.budget;
        div()
            .id(SharedString::from(format!("budget-{}", b.label())))
            .w(px(SEG_W))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(px(text::FOOTNOTE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if active { t.text } else { t.text_muted })
            .when(available, |el| {
                el.on_click(move |_, window, cx| {
                    picker.update(cx, |s, cx| {
                        s.budget = b;
                        cx.notify();
                    });
                    if let Some(f) = on_pick.as_ref() {
                        f(Some(b), window, cx);
                    }
                })
            })
            .child(b.label())
    });
    let budget = div()
        .pt(px(10.))
        .when(!available, |el| el.opacity(0.45))
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .relative()
                .flex_none()
                .w(px(SEG_W * 3. + 4.))
                .h(px(28.))
                .p(px(2.))
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Secondary))
                .child(
                    div()
                        .absolute()
                        .top(px(2.))
                        .left(px(2. + thumb_x))
                        .w(px(SEG_W))
                        .h(px(24.))
                        .rounded(px(radius::SM))
                        .bg(thumb)
                        .shadow(vec![shadow(
                            0.,
                            1.,
                            2.,
                            0.,
                            hsla(0., 0., 0., if t.dark { 0.4 } else { 0.12 }),
                        )]),
                )
                .child(div().relative().flex().size_full().children(segments)),
        )
        .child(
            div()
                .overflow_hidden()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child(roll(
                    format!("{key}-budget-note"),
                    s.budget.note().to_string(),
                    6.,
                )),
        );

    div()
        .px(px(14.))
        .pt(px(10.))
        .pb(px(10. + 2. * unfold))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .size(px(28.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::MD))
                        .bg(motion::mix(
                            t.fill(FillTier::Quaternary),
                            t.accent.opacity(if t.dark { 0.18 } else { 0.1 }),
                            lit,
                        ))
                        .child(
                            svg()
                                .path(ICON_HOURGLASS)
                                .size(px(16.))
                                .text_color(motion::mix(t.text_muted, t.accent, lit)),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(text::BODY))
                                .line_height(px(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Think longer"),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(note),
                        ),
                )
                .child(toggle),
        )
        .child(
            div()
                .h(px(38. * unfold))
                .overflow_hidden()
                .opacity(unfold)
                .child(budget),
        )
}

/// A 30×18 switch whose knob and track ease between off and on.
fn switch(key: &str, on: bool, t: &Luna) -> Stateful<gpui::Div> {
    let k = motion::follow(
        key,
        if on { 1. } else { 0. },
        MotionSpec::new(240, EASE_OUT_QUINT),
    );
    let off = if t.dark {
        t.fill(FillTier::Primary)
    } else {
        t.border_strong
    };
    div()
        .id(SharedString::from(key.to_string()))
        .relative()
        .flex_none()
        .w(px(30.))
        .h(px(18.))
        .rounded_full()
        .bg(motion::mix(off, t.accent_strong, k))
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(2. + 12. * k))
                .size(px(14.))
                .rounded_full()
                .bg(gpui::white())
                .shadow(vec![shadow(0., 1., 2., 0., hsla(0., 0., 0., 0.25))]),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Luna::get(cx);
        let hint: Hsla = t.text_faint;
        div()
            .h(px(420.))
            .w(px(600.))
            .flex()
            .flex_col()
            .justify_end()
            .font_family(luna::SANS)
            .child(
                luna::card(&t)
                    .w_full()
                    .rounded(px(radius::XL))
                    .p(px(8.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(44.))
                            .px(px(8.))
                            .pt(px(6.))
                            .text_size(px(text::CALLOUT))
                            .text_color(hint)
                            .child("Ask Helix to draft, review or explain…"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(luna::icon_button(
                                "mp-attach",
                                ICON_PLUS,
                                ControlSize::Md,
                                &t,
                            ))
                            .child(
                                ModelPicker::new("model-picker", default_models())
                                    .selected("helix-pro")
                                    .reasoning(true, Budget::Balanced)
                                    .default_open(true)
                                    .autofocus(true),
                            )
                            .child(div().flex_1())
                            .child(luna::icon_button("mp-voice", ICON_MIC, ControlSize::Md, &t))
                            .child(
                                luna::button("mp-send", ButtonKind::Primary, ControlSize::Md, &t)
                                    .w(px(28.))
                                    .px_0()
                                    .child(
                                        luna::glyph(ICON_SEND, ControlSize::Md)
                                            .text_color(t.on_solid),
                                    ),
                            ),
                    ),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_follow_the_budget() {
        let pro = ModelOption::new("pro", "Pro")
            .first_token(0.8, 0.8)
            .price(9., 9.);
        assert_eq!(pro.first_token_label(None), "0.8 s");
        assert_eq!(pro.first_token_label(Some(Budget::Balanced)), "4.8 s");
        assert_eq!(pro.reply_cost_label(None), "$0.0063");
        assert_eq!(pro.reply_cost_label(Some(Budget::Deep)), "$0.222");
    }

    #[test]
    fn routed_models_show_ranges() {
        let auto = ModelOption::new("auto", "Auto")
            .first_token(0.3, 1.6)
            .price(1.2, 30.);
        assert_eq!(auto.first_token_label(None), "0.3–1.6 s");
        assert!(auto.reply_cost_label(None).starts_with("≤ $"));
        assert_eq!(speed_word((0.4, 0.9)), "Adapts");
    }
}
