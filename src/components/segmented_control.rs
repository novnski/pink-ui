//! Segmented Control – a Pink component.
//!
//! Pink's segmented control (Pink: `button.rs` `segmented`): equal-width
//! options on a quiet `fill(Quaternary)` track with a hairline edge, and one
//! thumb that slides to the chosen option over `motion::QUICK` instead of each
//! option swapping its own fill. The chosen label is medium `text`; the others
//! rest in `readable_muted(0.9)` and brighten on hover. Three shapes share the
//! builder:
//!
//! - **Tray** (the default): the segmented control. Give it Pink's `width`,
//!   let it size every option to the widest one, or fill the container.
//! - **Buttons**: a row of Pink chips (Pink: `button.rs` `chip`) for a choice
//!   among more options; the chosen chip takes the accent wash and edge.
//! - **Underline**: a tab strip on a hairline (Pink: `pickers.rs` `model_tab`),
//!   the viewed tab marked by a 2 px accent capsule (`tab_indicator`).
//!
//! Every option is a tab stop with Pink's 2 px inset focus ring; arrow keys,
//! Home and End move and select. Options can carry a glyph, a count or a small
//! accent badge, and can be disabled.
//!
//! ```ignore
//! SegmentedControl::new("usage-metric", vec![
//!     SegmentOption::new("cost", "Cost"),
//!     SegmentOption::new("tokens", "Tokens"),
//!     SegmentOption::new("limits", "Limits"),
//! ])
//! .value(current)
//! .width(216.)
//! .on_change(|value, _, _| println!("{value}"))
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AnyView, App, BoxShadow, Div, ElementId, FocusHandle, FontWeight, Hsla,
    IntoElement, KeyDownEvent, MouseDownEvent, Role, SharedString, Stateful, Task, TextRun,
    Toggled, Window, div, font, point, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, QUICK},
    pink::{self, ControlSize, FillTier, Pink, layout, radius},
    story,
};

pub const NAME: &str = "Segmented Control";
pub const DESCRIPTION: &str = "Switch between two to five options: a thumb that slides along a quiet track, accent chips or tabs on a hairline, with glyphs, counts, badges and arrow keys";

type ChangeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// The tray's drawing, shared with PINK's `button.rs` `segmented`.
pub mod tray {
    include!("../../components/segmented-control/src/lib.rs");
}

use tray::{INSET, OPTION_GAP, OPTION_PAD_X};
/// Between chips (Pink: automations.rs, the editor's choice rows).
const CHIP_GAP: f32 = 6.0;
/// Between tabs, and how much taller the strip is than a tab (Pink: the model
/// menu's 32 px tabs in a 40 px row).
const TAB_GAP: f32 = 2.0;
const TAB_STRIP_EXTRA: f32 = 8.0;
/// The badge capsule (Pink: pickers.rs `new_model_badge`).
const BADGE_H: f32 = 16.0;
const BADGE_PAD_X: f32 = 5.0;
const BADGE_TEXT: f32 = 10.5;
const BADGE_LINE: f32 = 14.0;

/// One option of the control.
#[derive(Clone)]
pub struct SegmentOption {
    pub value: SharedString,
    pub label: Option<SharedString>,
    /// An SVG path under `assets/`, tinted like the label.
    pub icon: Option<SharedString>,
    /// A quiet number after the label, e.g. how many items the tab holds.
    pub count: Option<SharedString>,
    /// A small accent capsule after the label, e.g. "New".
    pub badge: Option<SharedString>,
    pub disabled: bool,
}

impl SegmentOption {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: Some(label.into()),
            icon: None,
            count: None,
            badge: None,
            disabled: false,
        }
    }

    /// An option that only shows an icon (announced by its value).
    pub fn icon_only(value: impl Into<SharedString>, icon: impl Into<SharedString>) -> Self {
        Self {
            label: None,
            icon: Some(icon.into()),
            ..Self::new(value, "")
        }
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn count(mut self, count: impl std::fmt::Display) -> Self {
        self.count = Some(count.to_string().into());
        self
    }

    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Pink's control sizes: 24 / 28 / 32 tall, 11 / 12 / 13 type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentSize {
    Sm,
    #[default]
    Md,
    Lg,
}

impl SegmentSize {
    fn control(self) -> ControlSize {
        match self {
            SegmentSize::Sm => ControlSize::Sm,
            SegmentSize::Md => ControlSize::Md,
            SegmentSize::Lg => ControlSize::Lg,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentVariant {
    /// Separate chips, the chosen one in the accent wash (Pink `chip`).
    Buttons,
    /// A track with a sliding thumb (Pink `segmented`).
    #[default]
    Tray,
    /// Tabs on a hairline, the viewed one underlined (Pink `model_tab`).
    Underline,
}

/// `radio` lets Up/Down move too; `tabs` only Left/Right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentKind {
    #[default]
    Radio,
    Tabs,
}

struct State {
    /// The value when uncontrolled.
    inner: Option<SharedString>,
    /// The latest pick, ahead of the next render: keys can pick twice
    /// between two frames.
    latest: Option<SharedString>,
    /// One handle per option, so arrow keys can move focus along.
    focus: Vec<FocusHandle>,
}

/// Picks options from clicks and keys.
struct Picker {
    state: gpui::Entity<State>,
    values: Vec<SharedString>,
    enabled: Vec<bool>,
    controlled: bool,
    on_change: Option<ChangeHandler>,
}

impl Picker {
    fn pick(&self, ix: usize, window: &mut Window, cx: &mut App) {
        if !self.enabled.get(ix).copied().unwrap_or(false) {
            return;
        }
        let value = self.values[ix].clone();
        let controlled = self.controlled;
        let changed = self.state.update(cx, |s, cx| {
            if s.latest.as_ref() == Some(&value) {
                return false;
            }
            s.latest = Some(value.clone());
            if !controlled {
                s.inner = Some(value.clone());
            }
            cx.notify();
            true
        });
        if changed && let Some(on_change) = &self.on_change {
            on_change(&value, window, cx);
        }
    }

    /// Arrow keys, Home and End: pick the next enabled option and focus it.
    fn key(&self, key: &str, kind: SegmentKind, window: &mut Window, cx: &mut App) -> bool {
        let enabled: Vec<usize> = (0..self.values.len())
            .filter(|&i| self.enabled[i])
            .collect();
        if enabled.is_empty() {
            return false;
        }
        let radio = kind == SegmentKind::Radio;
        let latest = self.state.read(cx).latest.clone();
        let len = enabled.len() as i32;
        let at = enabled
            .iter()
            .position(|&i| Some(&self.values[i]) == latest.as_ref())
            .map_or(-1, |i| i as i32);
        // With nothing picked yet, Left starts from the end and Right from the start
        let prev = enabled[if at < 0 {
            len - 1
        } else {
            (at - 1 + len) % len
        } as usize];
        let next = enabled[((at + 1) % len) as usize];
        let target = match key {
            "left" => prev,
            "up" if radio => prev,
            "right" => next,
            "down" if radio => next,
            "home" => enabled[0],
            "end" => enabled[enabled.len() - 1],
            _ => return false,
        };
        self.pick(target, window, cx);
        let handle = self.state.read(cx).focus.get(target).cloned();
        if let Some(handle) = handle {
            window.focus(&handle, cx);
        }
        true
    }
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

fn text_width(window: &Window, text: &SharedString, size: f32, weight: FontWeight) -> f32 {
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight,
            ..font(pink::SANS)
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.clone(), px(size), &[run], None);
    f32::from(line.width)
}

/// A track option's content at its widest: the label in the chosen (medium)
/// weight, so picking never truncates it.
fn content_width(window: &Window, o: &SegmentOption, size: ControlSize) -> f32 {
    let mut parts: Vec<f32> = Vec::new();
    if o.icon.is_some() {
        parts.push(ControlSize::Sm.glyph());
    }
    if let Some(label) = &o.label {
        parts.push(text_width(window, label, size.text(), FontWeight::MEDIUM));
    }
    if let Some(count) = &o.count {
        parts.push(text_width(window, count, size.text(), FontWeight::NORMAL));
    }
    if let Some(badge) = &o.badge {
        parts.push(text_width(window, badge, BADGE_TEXT, FontWeight::SEMIBOLD) + BADGE_PAD_X * 2.);
    }
    parts.iter().sum::<f32>() + parts.len().saturating_sub(1) as f32 * OPTION_GAP
}

/// The track width when the caller gives none: every option as wide as the
/// widest, plus the inset and the hairline edge.
fn natural_width(window: &Window, options: &[SegmentOption], size: ControlSize) -> f32 {
    let widest = options
        .iter()
        .map(|o| content_width(window, o, size))
        .fold(0., f32::max);
    let option = (widest + OPTION_PAD_X * 2. + 1.).ceil();
    option * options.len().max(1) as f32 + (INSET + 1.) * 2.
}

/// A quiet count after the label, in tabular figures (Pink: activity
/// section headings, `readable_faint(0.7)`).
fn count_text(count: SharedString, t: &Pink) -> Div {
    div()
        .flex_none()
        .font_weight(FontWeight::NORMAL)
        .font_features(pink::tabular_figures())
        .text_color(t.readable_faint(0.7))
        .child(count)
}

/// A small accent capsule (Pink: pickers.rs `new_model_badge`).
fn badge(label: SharedString, t: &Pink) -> Div {
    div()
        .flex_none()
        .h(px(BADGE_H))
        .px(px(BADGE_PAD_X))
        .flex()
        .items_center()
        .rounded(px(radius::XS))
        .bg(t.accent.opacity(0.14))
        .text_size(px(BADGE_TEXT))
        .line_height(px(BADGE_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.readable_tone(t.accent, 1.0))
        .child(label)
}

/// An option's glyph, label, count and badge, in order. Track labels
/// truncate inside their equal share; chips and tabs size to their content.
fn option_parts(
    o: &SegmentOption,
    glyph_size: f32,
    glyph_tone: Hsla,
    truncate: bool,
    t: &Pink,
) -> Vec<AnyElement> {
    let mut parts = Vec::new();
    if let Some(path) = o.icon.clone() {
        parts.push(
            svg()
                .path(path)
                .size(px(glyph_size))
                .flex_none()
                .text_color(glyph_tone)
                .into_any_element(),
        );
    }
    if let Some(label) = o.label.clone() {
        parts.push(if truncate {
            div().min_w_0().truncate().child(label).into_any_element()
        } else {
            label.into_any_element()
        });
    }
    if let Some(count) = o.count.clone() {
        parts.push(count_text(count, t).into_any_element());
    }
    if let Some(label) = o.badge.clone() {
        parts.push(badge(label, t).into_any_element());
    }
    parts
}

/// The 2 px capsule under the viewed tab, sitting on the strip's hairline
/// (Pink: pickers.rs `tab_indicator`).
fn tab_indicator(tint: Hsla) -> Div {
    div()
        .absolute()
        .bottom(px(-4.))
        .left(px(6.))
        .right(px(6.))
        .h(px(2.))
        .rounded(px(1.))
        .bg(tint)
}

/// What every shape needs to draw its options.
struct Shape {
    t: Pink,
    size: ControlSize,
    key: String,
    radio: bool,
    active: Option<usize>,
    handles: Vec<FocusHandle>,
    picker: Rc<Picker>,
    /// The whole control is disabled: it dims as one, not per option.
    disabled: bool,
}

impl Shape {
    fn option_key(&self, o: &SegmentOption) -> String {
        format!("{}-{}", self.key, o.value)
    }

    /// The option's id and its announced role and state.
    fn option(&self, o: &SegmentOption, chosen: bool) -> Stateful<Div> {
        self.announce(div().id(SharedString::from(self.option_key(o))), o, chosen)
    }

    /// An option's announced role, name and state.
    fn announce(&self, el: Stateful<Div>, o: &SegmentOption, chosen: bool) -> Stateful<Div> {
        let radio = self.radio;
        el.role(if radio { Role::RadioButton } else { Role::Tab })
            .aria_label(o.label.clone().unwrap_or_else(|| o.value.clone()))
            .map(|el| {
                if radio {
                    el.aria_toggled(toggled(chosen))
                } else {
                    el.aria_selected(chosen)
                }
            })
    }

    /// Its focus handle and click.
    fn wire(&self, el: Stateful<Div>, ix: usize) -> Stateful<Div> {
        let picker = self.picker.clone();
        el.track_focus(&self.handles[ix])
            .on_click(move |_, window, cx| picker.pick(ix, window, cx))
    }

    /// Pointer, the inset focus ring, and Enter/Space reaching `on_click`
    /// once (Pink `segmented` / `model_tab`).
    fn focusable(&self, el: Stateful<Div>) -> Stateful<Div> {
        let ring = self.t.focus_ring();
        el.cursor_pointer()
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .on_key_down(pink::swallow_activation_key)
    }

    /// A disabled option inside an enabled control: Pink's disabled controls
    /// paint at 45%.
    fn dim(&self, o: &SegmentOption) -> bool {
        o.disabled && !self.disabled
    }

    /// Pink: button.rs `segmented` – the track, one sliding thumb and
    /// equal-width options, drawn by `pink-segmented-control`. `width: None`
    /// fills the container.
    fn tray(&self, options: &[SegmentOption], width: Option<f32>) -> Div {
        let t = &self.t;
        let muted = t.readable_muted(0.9);
        let track_radius = self.size.radius();
        let items: Vec<AnyElement> = options
            .iter()
            .enumerate()
            .map(|(ix, o)| {
                let chosen = Some(ix) == self.active;
                let tone = if chosen { t.text } else { muted };
                let el = tray::option(SharedString::from(self.option_key(o)), chosen, track_radius);
                self.announce(el, o, chosen)
                    .map(|el| {
                        if self.picker.enabled[ix] {
                            self.focusable(self.wire(el, ix)).hover_fg(
                                self.option_key(o),
                                tone,
                                t.text,
                            )
                        } else {
                            el.text_color(tone)
                        }
                    })
                    .when(self.dim(o), |el| el.opacity(0.45))
                    .children(option_parts(o, ControlSize::Sm.glyph(), tone, true, t))
                    .into_any_element()
            })
            .collect();
        // Placed by fraction of the content box, so nothing is measured
        let thumb = self.active.map(|ix| {
            let position = motion::follow(&format!("{}-thumb", self.key), ix as f32, QUICK);
            tray::thumb(
                position,
                options.len(),
                track_radius,
                if t.dark {
                    t.fill(FillTier::Primary)
                } else {
                    gpui::white()
                },
                (!t.dark).then(|| tray::Lift {
                    edge: t.separator(false),
                    shadows: vec![BoxShadow {
                        color: gpui::hsla(0., 0., 0., 0.08),
                        offset: point(px(0.), px(1.)),
                        blur_radius: px(2.),
                        spread_radius: px(0.),
                        inset: false,
                    }],
                }),
            )
        });
        tray::tray(
            &tray::Track {
                width,
                height: self.size.height(),
                radius: track_radius,
                text: self.size.text(),
                fill: t.fill(FillTier::Quaternary),
                edge: t.separator(false),
            },
            thumb,
            items,
        )
    }

    /// Pink: button.rs `chip` – one chip per option, gap 6; `full` shares
    /// the row equally.
    fn chips(&self, options: &[SegmentOption], full: bool) -> Div {
        let t = &self.t;
        let glyph = t.glyph_tone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(CHIP_GAP))
            .map(|el| if full { el.w_full() } else { el.flex_none() })
            .children(options.iter().enumerate().map(|(ix, o)| {
                let chosen = Some(ix) == self.active;
                let enabled = self.picker.enabled[ix];
                pink::chip_enabled(self.option_key(o), chosen, self.size, enabled, t)
                    .role(if self.radio {
                        Role::RadioButton
                    } else {
                        Role::Tab
                    })
                    .aria_label(o.label.clone().unwrap_or_else(|| o.value.clone()))
                    .when(!self.radio, |el| el.aria_selected(chosen))
                    .when(full, |el| el.flex_1().min_w_0())
                    .when(enabled, |el| self.wire(el, ix))
                    .children(option_parts(o, self.size.glyph(), glyph, false, t))
            }))
    }

    /// Pink: pickers.rs `model_tab` – tabs on a hairline strip, the viewed
    /// one in `text` with the accent capsule, the others `text_muted` with
    /// the quiet hover wash. Every tab is medium, so picking never reflows
    /// the strip. `full` spans the container; tabs keep their size.
    fn tabs(&self, options: &[SegmentOption], full: bool) -> Div {
        let t = &self.t;
        let size = self.size;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(TAB_GAP))
            .px(px(layout::CARD_INSET))
            .h(px(size.height() + TAB_STRIP_EXTRA))
            .border_b_1()
            .border_color(t.hairline(0.08))
            .map(|el| if full { el.w_full() } else { el.flex_none() })
            .children(options.iter().enumerate().map(|(ix, o)| {
                let chosen = Some(ix) == self.active;
                let enabled = self.picker.enabled[ix];
                let tone = if chosen { t.text } else { t.text_muted };
                self.option(o, chosen)
                    .relative()
                    .flex_none()
                    .h(px(size.height()))
                    .px(px(size.pad_x()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(size.gap()))
                    .rounded(px(size.radius()))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tone)
                    .when(enabled, |el| self.focusable(self.wire(el, ix)))
                    .when(enabled && !chosen, |el| {
                        el.hover_wash(self.option_key(o), t.fill(FillTier::Tertiary))
                    })
                    .when(self.dim(o), |el| el.opacity(0.45))
                    .children(option_parts(o, size.glyph(), tone, false, t))
                    .when(chosen, |el| el.child(tab_indicator(t.accent)))
            }))
    }
}

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    options: Vec<SegmentOption>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    on_change: Option<ChangeHandler>,
    size: SegmentSize,
    variant: SegmentVariant,
    width: Option<f32>,
    full_width: bool,
    kind: SegmentKind,
    disabled: bool,
}

impl SegmentedControl {
    pub fn new(id: impl Into<ElementId>, options: Vec<SegmentOption>) -> Self {
        Self {
            id: id.into(),
            options,
            value: None,
            default_value: None,
            on_change: None,
            size: SegmentSize::Md,
            variant: SegmentVariant::Tray,
            width: None,
            full_width: false,
            kind: SegmentKind::Radio,
            disabled: false,
        }
    }

    /// Controlled value: the control shows it and reports picks through
    /// [`on_change`](Self::on_change).
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Starting value when uncontrolled – defaults to the first enabled option.
    pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
        self.default_value = Some(value.into());
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn size(mut self, size: SegmentSize) -> Self {
        self.size = size;
        self
    }

    /// Tray (the default), Buttons or Underline.
    pub fn variant(mut self, variant: SegmentVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The tray's whole width, shared equally by its options (Pink's
    /// `width`). Without it every option is as wide as the widest.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Fill the container: tray options and chips share it equally; tabs
    /// keep their size on a hairline that spans it.
    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    pub fn kind(mut self, kind: SegmentKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let size = self.size.control();
        let options = self.options;
        let count = options.len();
        let radio = self.kind == SegmentKind::Radio;

        let first_enabled = options
            .iter()
            .find(|o| !o.disabled)
            .map(|o| o.value.clone());
        let initial = self.default_value.clone().or(first_enabled);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State {
                inner: initial,
                latest: None,
                focus: Vec::new(),
            },
        );
        let controlled = self.value.is_some();
        let current = self.value.clone().or_else(|| state.read(cx).inner.clone());
        let active = options
            .iter()
            .position(|o| Some(&o.value) == current.as_ref());
        let handles = state.update(cx, |s, cx| {
            while s.focus.len() < count {
                s.focus.push(cx.focus_handle().tab_stop(true));
            }
            s.focus.truncate(count);
            s.latest = current.clone();
            s.focus.clone()
        });

        let picker = Rc::new(Picker {
            state: state.clone(),
            values: options.iter().map(|o| o.value.clone()).collect(),
            enabled: options
                .iter()
                .map(|o| !o.disabled && !self.disabled)
                .collect(),
            controlled,
            on_change: self.on_change.clone(),
        });
        let shape = Shape {
            t: t.clone(),
            size,
            key: format!("seg-{}", self.id),
            radio,
            active,
            handles,
            picker: picker.clone(),
            disabled: self.disabled,
        };
        let body = match self.variant {
            SegmentVariant::Tray => {
                let width = (!self.full_width).then(|| {
                    self.width
                        .unwrap_or_else(|| natural_width(window, &options, size))
                });
                shape.tray(&options, width)
            }
            SegmentVariant::Buttons => shape.chips(&options, self.full_width),
            SegmentVariant::Underline => shape.tabs(&options, self.full_width),
        };

        let on_key = {
            let kind = self.kind;
            move |e: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if e.keystroke.modifiers.modified() {
                    return;
                }
                if picker.key(e.keystroke.key.as_str(), kind, window, cx) {
                    cx.stop_propagation();
                }
            }
        };

        motion::keep_alive(window);
        body.id(self.id)
            .role(if radio {
                Role::RadioGroup
            } else {
                Role::TabList
            })
            .font_family(pink::SANS)
            // The tray sets its own rem type size, so it grows with the
            // Interface size as PINK's does.
            .when(self.variant != SegmentVariant::Tray, |el| {
                el.text_size(px(size.text()))
            })
            .line_height(px(size.line_height()))
            .text_color(t.text)
            .on_key_down(on_key)
            // Chips dim one by one (Pink `chip` with `enabled: false`)
            .when(
                self.disabled && self.variant != SegmentVariant::Buttons,
                |el| el.opacity(0.45),
            )
    }
}

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

const ICON_LAYERS: &str = "icons/segmented-control/layers-minimalistic.svg";
const ICON_CALENDAR: &str = "icons/segmented-control/calendar.svg";

/// One step of the tour the Usage card takes after load.
#[derive(Clone, Copy)]
enum Pick {
    Metric(&'static str),
    Period(&'static str),
}

/// Picked in turn after load, until someone interacts.
const TOUR: [Pick; 4] = [
    Pick::Metric("tokens"),
    Pick::Period("30"),
    Pick::Metric("cost"),
    Pick::Period("7"),
];
/// Between two tour steps.
const TOUR_STEP: Duration = Duration::from_millis(800);

struct DemoState {
    tab: SharedString,
    metric: SharedString,
    period: SharedString,
    breakdown: SharedString,
    alert: SharedString,
    tour: Option<Task<()>>,
}

impl DemoState {
    fn new(cx: &mut gpui::Context<Self>) -> Self {
        let tour = cx.spawn(async move |this, cx| {
            for pick in TOUR {
                cx.background_executor().timer(TOUR_STEP).await;
                let updated = this.update(cx, |s, cx| {
                    match pick {
                        Pick::Metric(value) => s.metric = value.into(),
                        Pick::Period(value) => s.period = value.into(),
                    }
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        });
        Self {
            tab: "overview".into(),
            metric: "cost".into(),
            period: "7".into(),
            breakdown: "model".into(),
            alert: "80".into(),
            tour: Some(tour),
        }
    }
}

#[derive(IntoElement)]
struct Demo;

impl RenderOnce for Demo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let state =
            window.use_keyed_state("segmented-control-demo", cx, |_, cx| DemoState::new(cx));
        let s = state.read(cx);
        let (tab, metric, period, breakdown, alert) = (
            s.tab.clone(),
            s.metric.clone(),
            s.period.clone(),
            s.breakdown.clone(),
            s.alert.clone(),
        );
        let hourly = period == "1";

        // Any press or key ends the tour
        let stop_mouse = {
            let state = state.clone();
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| s.tour = None);
            }
        };
        let stop_key = {
            let state = state.clone();
            move |_: &KeyDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| s.tour = None);
            }
        };
        let setter = |field: fn(&mut DemoState) -> &mut SharedString| {
            let state = state.clone();
            move |value: &SharedString, _: &mut Window, cx: &mut App| {
                let value = value.clone();
                state.update(cx, |s, cx| {
                    *field(s) = value;
                    cx.notify();
                });
            }
        };
        // A settings row: its text, then the control (Pink: settings/widgets.rs)
        let row = |first: bool, title: &'static str, copy: &'static str, control: AnyElement| {
            pink::card_row(first, &t)
                .child(pink::row_text(title, Some(copy.into()), &t))
                .child(control)
        };

        let tabs = vec![
            SegmentOption::new("overview", "Overview"),
            SegmentOption::new("models", "Models").count(6),
            SegmentOption::new("machines", "Machines").count(2),
            SegmentOption::new("budgets", "Budgets").badge("New"),
        ];
        let metrics = vec![
            SegmentOption::new("cost", "Cost"),
            SegmentOption::new("tokens", "Tokens"),
            SegmentOption::new("limits", "Limits"),
        ];
        let periods = vec![
            SegmentOption::new("1", "Past 24h"),
            SegmentOption::new("7", "7 days"),
            SegmentOption::new("30", "30 days"),
            SegmentOption::new("90", "90 days"),
        ];
        let breakdowns = vec![
            SegmentOption::new("model", "Model").icon(ICON_LAYERS),
            SegmentOption::new("time", if hourly { "Hour" } else { "Day" }).icon(ICON_CALENDAR),
        ];
        let alerts = vec![
            SegmentOption::new("off", "Off"),
            SegmentOption::new("80", "At 80%"),
            SegmentOption::new("95", "At 95%"),
            SegmentOption::new("over", "Over budget").disabled(true),
        ];

        pink::section_card(&t)
            .mt_0()
            .w(px(600.))
            .font_family(pink::SANS)
            .capture_any_mouse_down(stop_mouse)
            .capture_key_down(stop_key)
            .child(pink::section_header(
                "Usage",
                Some("What your providers report on this machine.".into()),
                &t,
            ))
            .child(
                SegmentedControl::new("usage-tab", tabs)
                    .variant(SegmentVariant::Underline)
                    .kind(SegmentKind::Tabs)
                    .full_width(true)
                    .value(tab)
                    .on_change(setter(|s| &mut s.tab)),
            )
            .child(row(
                true,
                "Show",
                "Estimated API cost, processed tokens, or the subscription limits your providers report.",
                pink::row_controls()
                    .child(
                        SegmentedControl::new("usage-metric", metrics)
                            .width(216.)
                            .value(metric)
                            .on_change(setter(|s| &mut s.metric)),
                    )
                    .into_any_element(),
            ))
            .child(row(
                false,
                "Period",
                if hourly {
                    "One point per hour."
                } else {
                    "One point per day."
                },
                pink::row_controls()
                    .child(
                        SegmentedControl::new("usage-days", periods)
                            .width(300.)
                            .value(period)
                            .on_change(setter(|s| &mut s.period)),
                    )
                    .into_any_element(),
            ))
            .child(row(
                false,
                "Breakdown",
                "Split the table by model or by period.",
                pink::row_controls()
                    .child(
                        SegmentedControl::new("usage-breakdown", breakdowns)
                            .width(150.)
                            .value(breakdown)
                            .on_change(setter(|s| &mut s.breakdown)),
                    )
                    .into_any_element(),
            ))
            .child(row(
                false,
                "Budget alerts",
                "A notice as this month's spend nears the budget. Over budget needs a monthly budget.",
                pink::row_block()
                    .child(
                        SegmentedControl::new("usage-alerts", alerts)
                            .variant(SegmentVariant::Buttons)
                            .size(SegmentSize::Sm)
                            .value(alert)
                            .on_change(setter(|s| &mut s.alert)),
                    )
                    .into_any_element(),
            ))
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| Demo)
}
