//! Segmented Control – switch between two to six options.
//!
//! Three variants:
//! - **Buttons** (the Clean default): separate quiet chips with a hairline;
//!   a light-gray fill slides to the active one.
//! - **Tray** (the Classic default): a gray track with a white raised pill.
//! - **Underline**: text tabs on a hairline with a sliding line.
//!
//! The indicator's left and right edges glide separately (`motion::follow`):
//! the edge facing the target leads, the other trails, so it stretches a
//! little and settles. Arrow keys, Home and End move and select; press an
//! option and drag across the others to pick as you go (the pill gives a
//! little while held). Options are sized to their content, so a changed
//! label or count makes the control morph to its new size. Classic labels
//! crossfade to semibold without the width jumping.
//!
//! ```ignore
//! SegmentedControl::new("section", vec![
//!     SegmentOption::new("overview", "Overview"),
//!     SegmentOption::new("inbox", "Inbox").count(3),
//! ])
//! .value(current)
//! .on_change(|value, _, _| println!("{value}"))
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AnyView, App, BoxShadow, DispatchPhase, Entity, FocusHandle, FontWeight,
    HitboxBehavior, Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, SharedString, Task, TextRun, Window, canvas, div, font, prelude::*, px, svg,
};

use crate::{
    ActiveTheme, Look,
    motion::{self, CubicBezier, EASE, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, ring, shadow, white_a},
    story,
};

pub const NAME: &str = "Segmented Control";
pub const DESCRIPTION: &str = "Switch between two to six options – a raised pill or underline that glides, stretches a little and can be dragged across, with icons, counts and badges";

type ChangeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// `--ui-segc-ease`: short and crisp.
const CLEAN_EASE: CubicBezier = CubicBezier::new(0.25, 0.9, 0.3, 1.0);

/// One option of the control.
#[derive(Clone)]
pub struct SegmentOption {
    pub value: SharedString,
    pub label: Option<SharedString>,
    /// An SVG path under `assets/`, tinted like the label.
    pub icon: Option<SharedString>,
    /// A small gray number after the label, e.g. unread items.
    pub count: Option<SharedString>,
    /// A small tinted tag after the label, e.g. "Save 20%".
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

    /// An option that only shows an icon.
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentSize {
    Sm,
    #[default]
    Md,
    Lg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentVariant {
    /// Separate chips, the active one filled (Clean only; Classic draws a tray).
    Buttons,
    /// A track with a raised pill.
    Tray,
    /// Text tabs with a sliding line.
    Underline,
}

/// `radio` lets Up/Down move too; `tabs` only Left/Right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentKind {
    #[default]
    Radio,
    Tabs,
}

/// The `--ui-segc-*` / `--ui-seg-*` tokens.
struct Palette {
    ink: Hsla,
    /// Option text at rest.
    rest: Hsla,
    /// Option text while hovered (buttons/tray).
    hover: Hsla,
    /// Option text while hovered (underline).
    hover_line: Hsla,
    /// Clean draws icons in their own gray; Classic icons follow the text.
    icon: Option<Hsla>,
    line: Hsla,
    /// Hover wash behind an inactive chip (Clean only).
    wash: Option<Hsla>,
    /// The buttons variant's indicator.
    fill: Hsla,
    track: Hsla,
    pill: Hsla,
    pill_shadow: Vec<BoxShadow>,
    accent: Hsla,
    accent_soft: Hsla,
    count_bg: Hsla,
    count_fg: Hsla,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => Palette {
                ink: black_a(0.88),
                rest: black_a(0.61),
                hover: black_a(0.61),
                hover_line: black_a(0.75),
                icon: Some(black_a(0.447)),
                line: black_a(0.06),
                wash: Some(black_a(0.03)),
                fill: black_a(0.06),
                track: black_a(0.04),
                pill: hex(0xffffff),
                pill_shadow: vec![
                    ring(0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                accent: hexa(0x0060e2, 0.765),
                accent_soft: hexa(0x0060e2, 0.08),
                count_bg: black_a(0.06),
                count_fg: black_a(0.61),
                focus: hexa(0x0060e2, 0.35),
            },
            (Look::Clean, true) => Palette {
                ink: white_a(0.9),
                rest: white_a(0.62),
                hover: white_a(0.62),
                hover_line: white_a(0.78),
                icon: Some(white_a(0.46)),
                line: white_a(0.08),
                wash: Some(white_a(0.04)),
                fill: white_a(0.08),
                track: white_a(0.05),
                pill: hex(0x2c2c2f),
                pill_shadow: vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                accent: hex(0x4d8dff),
                accent_soft: hexa(0x4d8dff, 0.14),
                count_bg: white_a(0.08),
                count_fg: white_a(0.62),
                focus: hexa(0x4d8dff, 0.45),
            },
            (Look::Classic, false) => Palette {
                ink: hex(0x171717),
                rest: hex(0x737373),
                hover: hex(0x404040),
                hover_line: hex(0x404040),
                icon: None,
                line: hex(0xececec),
                wash: None,
                fill: hex(0xffffff),
                track: hex(0xf0f0f0),
                pill: hex(0xffffff),
                pill_shadow: vec![
                    ring(0.5, black_a(0.08)),
                    shadow(0., 1., 2., 0., black_a(0.08)),
                    shadow(0., 2., 8., -2., black_a(0.06)),
                ],
                accent: hex(0x2f6bff),
                accent_soft: hexa(0x2f6bff, 0.1),
                count_bg: black_a(0.06),
                count_fg: hex(0x404040),
                focus: hexa(0x171717, 0.22),
            },
            (Look::Classic, true) => Palette {
                ink: hex(0xededed),
                rest: hex(0x8b8b92),
                hover: hex(0xc8c8cc),
                hover_line: hex(0xc8c8cc),
                icon: None,
                line: hex(0x2a2a2e),
                wash: None,
                fill: hex(0x26262a),
                track: hex(0x111113),
                pill: hex(0x26262a),
                pill_shadow: vec![
                    ring(1., white_a(0.06)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                accent: hex(0x6d8bff),
                accent_soft: hexa(0x6d8bff, 0.14),
                count_bg: white_a(0.08),
                count_fg: hex(0xc8c8cc),
                focus: white_a(0.3),
            },
        }
    }
}

/// Sizes per look × size, straight from the two stylesheets.
#[derive(Clone, Copy)]
struct Metrics {
    /// `--h`: option height (tray options shrink by the padding in Clean).
    h: f32,
    /// Horizontal option padding.
    pad_x: f32,
    font: f32,
    line: f32,
    icon: f32,
    /// Gap between buttons (Clean) or underline tabs (Classic).
    gap: f32,
    /// Tray padding.
    pad: f32,
    /// Tray corner radius (options use `radius - pad`).
    radius: f32,
    content_gap: f32,
    label_weight: FontWeight,
    count_min: f32,
    count_pad: f32,
    count_radius: f32,
    count_font: f32,
    count_line: f32,
    count_weight: FontWeight,
    badge_h: f32,
    badge_pad: f32,
    badge_radius: f32,
    badge_font: f32,
    /// Underline thickness and the hairline under the tabs.
    underline: f32,
    hairline: f32,
    /// Indicator glide: leading and trailing edge durations.
    lead: u64,
    trail: u64,
    press: u64,
    color: u64,
    curve: CubicBezier,
    /// The count's entrance: duration and rise.
    count_in: u64,
    count_rise: f32,
    /// Outline offset + width outside the option (`outline-offset` + 2px).
    focus_out: f32,
}

impl Metrics {
    fn get(look: Look, size: SegmentSize) -> Self {
        let sm = size == SegmentSize::Sm;
        match look {
            Look::Clean => Metrics {
                h: match size {
                    SegmentSize::Sm => 24.,
                    SegmentSize::Md => 28.,
                    SegmentSize::Lg => 32.,
                },
                pad_x: match size {
                    SegmentSize::Sm => 8.,
                    SegmentSize::Md => 10.,
                    SegmentSize::Lg => 12.,
                },
                font: if sm { 12. } else { 13. },
                line: if sm { 16. } else { 20. },
                icon: if sm { 14. } else { 16. },
                gap: if sm { 6. } else { 8. },
                pad: 2.,
                radius: 8.,
                content_gap: if sm { 5. } else { 6. },
                label_weight: FontWeight::MEDIUM,
                count_min: if sm { 14. } else { 16. },
                count_pad: if sm { 3. } else { 4. },
                count_radius: if sm { 4. } else { 5. },
                count_font: if sm { 10. } else { 11. },
                count_line: if sm { 14. } else { 16. },
                count_weight: FontWeight::MEDIUM,
                badge_h: if sm { 16. } else { 18. },
                badge_pad: if sm { 5. } else { 6. },
                badge_radius: 5.,
                badge_font: if sm { 10. } else { 11. },
                underline: 1.5,
                hairline: 0.5,
                lead: 160,
                trail: 220,
                press: 120,
                color: 90,
                curve: CLEAN_EASE,
                count_in: 200,
                count_rise: 2.,
                focus_out: 3.,
            },
            Look::Classic => Metrics {
                h: match size {
                    SegmentSize::Sm => 26.,
                    SegmentSize::Md => 30.,
                    SegmentSize::Lg => 36.,
                },
                pad_x: match size {
                    SegmentSize::Sm => 10.,
                    SegmentSize::Md => 12.,
                    SegmentSize::Lg => 16.,
                },
                font: match size {
                    SegmentSize::Sm => 12.5,
                    SegmentSize::Md => 13.5,
                    SegmentSize::Lg => 14.,
                },
                line: match size {
                    SegmentSize::Sm => 12.5,
                    SegmentSize::Md => 13.5,
                    SegmentSize::Lg => 14.,
                },
                icon: match size {
                    SegmentSize::Sm => 14.,
                    SegmentSize::Md => 15.,
                    SegmentSize::Lg => 16.,
                },
                gap: match size {
                    SegmentSize::Sm => 16.,
                    SegmentSize::Md => 20.,
                    SegmentSize::Lg => 24.,
                },
                pad: match size {
                    SegmentSize::Sm => 2.,
                    SegmentSize::Md => 3.,
                    SegmentSize::Lg => 4.,
                },
                radius: match size {
                    SegmentSize::Sm => 9.,
                    SegmentSize::Md => 11.,
                    SegmentSize::Lg => 13.,
                },
                content_gap: 6.,
                label_weight: FontWeight::MEDIUM,
                count_min: if sm { 16. } else { 18. },
                count_pad: if sm { 4. } else { 5. },
                count_radius: 9.,
                count_font: if sm { 10.5 } else { 11. },
                count_line: if sm { 16. } else { 18. },
                count_weight: FontWeight::SEMIBOLD,
                badge_h: 18.,
                badge_pad: 6.,
                badge_radius: 6.,
                badge_font: 11.,
                underline: 2.,
                hairline: 1.,
                lead: 270,
                trail: 400,
                press: 200,
                color: 200,
                curve: EASE_OUT_QUINT,
                count_in: 260,
                count_rise: 3.,
                focus_out: 2.,
            },
        }
    }
}

/// A press on the track: where it started and whether it has moved yet.
#[derive(Clone, Copy)]
struct Press {
    x: f32,
    moved: bool,
}

struct State {
    /// The value when uncontrolled.
    inner: Option<SharedString>,
    /// The latest pick, ahead of the next render: dragging picks several
    /// options between two frames.
    latest: Option<SharedString>,
    /// Where the indicator was last placed, to know which edge leads.
    placed: Option<usize>,
    toward_right: bool,
    press: Option<Press>,
    /// Focus came from the keyboard, so the focus ring shows.
    keyboard: bool,
    focus: Vec<FocusHandle>,
    /// The track's width, measured after layout (full-width controls only).
    track_w: f32,
}

/// Picks options from clicks, keys and drags.
struct Picker {
    state: Entity<State>,
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
        if !changed {
            return;
        }
        // Focus follows the pick when it was inside the control
        let handles = self.state.read(cx).focus.clone();
        if handles.iter().any(|h| h.is_focused(window))
            && let Some(handle) = handles.get(ix)
        {
            window.focus(handle, cx);
        }
        if let Some(on_change) = &self.on_change {
            on_change(&value, window, cx);
        }
    }

    /// Arrow keys, Home and End.
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
        let target = match key {
            "left" => enabled[((at - 1 + len) % len) as usize],
            "up" if radio => enabled[((at - 1 + len) % len) as usize],
            "right" => enabled[((at + 1) % len) as usize],
            "down" if radio => enabled[((at + 1) % len) as usize],
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

/// The option under (or nearest to) `x`, in track coordinates.
fn option_at(rects: &[(f32, f32)], x: f32) -> Option<usize> {
    let mut best = None;
    let mut dist = f32::INFINITY;
    for (i, &(left, width)) in rects.iter().enumerate() {
        let right = left + width;
        let d = if x < left {
            left - x
        } else if x > right {
            x - right
        } else {
            0.
        };
        if d < dist {
            dist = d;
            best = Some(i);
        }
    }
    best
}

fn text_width(window: &Window, text: &SharedString, size: f32, weight: FontWeight) -> f32 {
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight,
            ..font(SANS)
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

/// The content's width, rounded to whole pixels.
fn content_width(window: &Window, o: &SegmentOption, look: Look, m: &Metrics) -> f32 {
    let mut parts: Vec<f32> = Vec::new();
    if o.icon.is_some() {
        parts.push(m.icon);
    }
    if let Some(label) = &o.label {
        let w = text_width(window, label, m.font, m.label_weight);
        parts.push(match look {
            // The semibold copy reserves the width
            Look::Classic => w.max(text_width(window, label, m.font, FontWeight::SEMIBOLD)),
            Look::Clean => w,
        });
    }
    if let Some(count) = &o.count {
        let w = text_width(window, count, m.count_font, m.count_weight) + m.count_pad * 2.;
        parts.push(w.max(m.count_min));
    }
    if let Some(badge) = &o.badge {
        parts.push(text_width(window, badge, m.badge_font, FontWeight::MEDIUM) + m.badge_pad * 2.);
    }
    let gaps = parts.len().saturating_sub(1) as f32 * m.content_gap;
    (parts.iter().sum::<f32>() + gaps).round()
}

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: gpui::ElementId,
    options: Vec<SegmentOption>,
    value: Option<SharedString>,
    default_value: Option<SharedString>,
    on_change: Option<ChangeHandler>,
    size: SegmentSize,
    variant: Option<SegmentVariant>,
    full_width: bool,
    kind: SegmentKind,
    disabled: bool,
    look: Option<Look>,
}

impl SegmentedControl {
    pub fn new(id: impl Into<gpui::ElementId>, options: Vec<SegmentOption>) -> Self {
        Self {
            id: id.into(),
            options,
            value: None,
            default_value: None,
            on_change: None,
            size: SegmentSize::Md,
            variant: None,
            full_width: false,
            kind: SegmentKind::Radio,
            disabled: false,
            look: None,
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

    /// Defaults to Buttons in the Clean look and Tray in the Classic one.
    pub fn variant(mut self, variant: SegmentVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    /// Fill the container with equal segments.
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

    /// Overrides the global Clean/Classic look for this control.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let variant = match (look, self.variant) {
            (Look::Clean, None) => SegmentVariant::Buttons,
            (Look::Classic, None | Some(SegmentVariant::Buttons)) => SegmentVariant::Tray,
            (_, Some(v)) => v,
        };
        let underline = variant == SegmentVariant::Underline;
        let tray = variant == SegmentVariant::Tray;
        let buttons = variant == SegmentVariant::Buttons;
        let p = Palette::get(look, cx);
        let m = Metrics::get(look, self.size);
        let key = format!("seg-{}", self.id);
        let options = self.options;
        let count = options.len();
        let full = self.full_width;

        let first_enabled = options
            .iter()
            .find(|o| !o.disabled)
            .map(|o| o.value.clone());
        let initial = self.default_value.clone().or(first_enabled);
        let state = window.use_keyed_state(
            gpui::ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State {
                inner: initial,
                latest: None,
                placed: None,
                toward_right: true,
                press: None,
                keyboard: false,
                focus: Vec::new(),
                track_w: 0.,
            },
        );

        let controlled = self.value.is_some();
        let current = self.value.clone().or_else(|| state.read(cx).inner.clone());
        let active = options
            .iter()
            .position(|o| Some(&o.value) == current.as_ref());
        // One focus handle per option; the latest value; which edge leads
        let (toward_right, pressed, keyboard, handles, track_w) = state.update(cx, |s, cx| {
            while s.focus.len() < count {
                s.focus.push(cx.focus_handle());
            }
            s.focus.truncate(count);
            s.latest = current.clone();
            if s.placed != active {
                if let (Some(from), Some(to)) = (s.placed, active)
                    && from != to
                {
                    s.toward_right = to > from;
                }
                s.placed = active;
            }
            (
                s.toward_right,
                s.press.is_some(),
                s.keyboard,
                s.focus.clone(),
                s.track_w,
            )
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

        // Geometry: every option as wide as its content plus padding
        let pad_x = match (underline, full) {
            (true, false) => 2.,
            (true, true) => 10.,
            _ => m.pad_x,
        };
        let gap = match variant {
            SegmentVariant::Buttons => m.gap,
            SegmentVariant::Underline if look == Look::Clean && !full => 20.,
            SegmentVariant::Underline if !full => m.gap,
            _ => 0.,
        };
        let opt_h = match variant {
            SegmentVariant::Tray if look == Look::Clean => m.h - m.pad * 2.,
            SegmentVariant::Underline => m.h + 6.,
            _ => m.h,
        };
        let contents: Vec<f32> = options
            .iter()
            .map(|o| content_width(window, o, look, &m))
            .collect();
        let natural: Vec<f32> = contents.iter().map(|c| c + pad_x * 2.).collect();
        let widths: Vec<f32> = if full {
            let widest = natural.iter().copied().fold(0., f32::max);
            let share = (track_w - gap * count.saturating_sub(1) as f32) / count.max(1) as f32;
            vec![share.max(widest); count]
        } else {
            natural
        };
        let mut rects = Vec::with_capacity(count);
        let mut x = 0.;
        for w in &widths {
            rects.push((x, *w));
            x += w + gap;
        }
        let rects = Rc::new(rects);

        // The indicator: both edges follow the active option, the one facing it leads
        let measured = !full || track_w > 0.;
        let pill = active.filter(|_| measured).map(|ix| {
            let (mut left, mut width) = rects[ix];
            if underline {
                // The line sits under the content only
                left += (width - contents[ix]) / 2.;
                width = contents[ix];
            }
            let (left_ms, right_ms) = if toward_right {
                (m.trail, m.lead)
            } else {
                (m.lead, m.trail)
            };
            let l = motion::follow(
                &format!("{key}-pill-l"),
                left,
                MotionSpec::new(left_ms, m.curve),
            );
            let r = motion::follow(
                &format!("{key}-pill-r"),
                left + width,
                MotionSpec::new(right_ms, m.curve),
            );
            let press = motion::follow(
                &format!("{key}-press"),
                if pressed && !underline { 1. } else { 0. },
                MotionSpec::new(m.press, m.curve),
            );
            // `scale: 0.97` while held, as an inset
            let inset_x = (r - l) * 0.015 * press;
            let inset_y = opt_h * 0.015 * press;
            let base = div()
                .absolute()
                .left(px(l + inset_x))
                .w(px((r - l - inset_x * 2.).max(0.)));
            if underline {
                base.bottom_0()
                    .h(px(m.underline))
                    .rounded_tl(px(m.underline))
                    .rounded_tr(px(m.underline))
                    .bg(p.ink)
            } else {
                let radius = match look {
                    Look::Clean => 6.,
                    Look::Classic => m.radius - m.pad,
                };
                base.top(px(inset_y))
                    .bottom(px(inset_y))
                    .rounded(px(radius))
                    .when(buttons, |d| d.bg(p.fill))
                    .when(tray, |d| d.bg(p.pill).shadow(p.pill_shadow.clone()))
            }
        });

        let width_spec = MotionSpec::new(if look == Look::Clean { 200 } else { 320 }, m.curve);
        let color_spec = MotionSpec::new(m.color, EASE);
        let opt_radius = match (look, variant) {
            (Look::Classic, SegmentVariant::Tray) => m.radius - m.pad,
            _ => 6.,
        };
        let focus_out = if underline { 0. } else { m.focus_out };
        // Options glide to a new width when their content changes
        let shown: Vec<f32> = options
            .iter()
            .zip(&widths)
            .map(|(o, &w)| {
                if full {
                    w
                } else {
                    motion::follow(&format!("{key}-w-{}", o.value), w, width_spec)
                }
            })
            .collect();
        // The focus ring sits on the track: options clip their content
        let ring = handles
            .iter()
            .position(|h| h.is_focused(window))
            .filter(|_| keyboard)
            .map(|ix| {
                let x = shown[..ix].iter().sum::<f32>() + gap * ix as f32;
                div()
                    .absolute()
                    .left(px(x - focus_out))
                    .top(px(-focus_out))
                    .w(px(shown[ix] + focus_out * 2.))
                    .h(px(opt_h + focus_out * 2.))
                    .border(px(2.))
                    .border_color(p.focus)
                    .rounded(px(opt_radius + focus_out))
            });

        let rows: Vec<AnyElement> = options
            .iter()
            .enumerate()
            .map(|(ix, o)| {
                let is_active = Some(ix) == active;
                let enabled = picker.enabled[ix];
                let a = motion::follow(
                    &format!("{key}-a-{}", o.value),
                    if is_active { 1. } else { 0. },
                    color_spec,
                );
                let hover_key = format!("{key}-h-{}", o.value);
                let h = if enabled {
                    motion::hover_t(&hover_key) * (1. - a)
                } else {
                    0.
                };
                let hover_fg = if underline { p.hover_line } else { p.hover };
                let fg = motion::mix(motion::mix(p.rest, hover_fg, h), p.ink, a);
                let icon_color = p.icon.map_or(fg, |icon| motion::mix(icon, p.ink, a));
                let content = option_content(o, look, &m, &p, is_active, a, icon_color);
                let picker = picker.clone();

                div()
                    .id(("opt", ix))
                    .track_focus(&handles[ix])
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(shown[ix]))
                    .h(px(opt_h))
                    .overflow_hidden()
                    .px(px(pad_x))
                    .rounded(px(opt_radius))
                    .whitespace_nowrap()
                    .text_color(fg)
                    .when(buttons, |d| d.border(px(0.5)).border_color(p.line))
                    .when_some(p.wash.filter(|_| !underline), |d, wash| {
                        d.bg(motion::mix(wash.opacity(0.), wash, h))
                    })
                    .on_hover(motion::hover_listener(hover_key))
                    .when(enabled, |d| {
                        d.cursor_pointer()
                            .on_click(move |_, window, cx| picker.pick(ix, window, cx))
                    })
                    .when(o.disabled, |d| d.opacity(0.4).cursor_not_allowed())
                    .child(content)
                    .into_any_element()
            })
            .collect();

        // Pointer: press an option, then drag across the others to pick as you go
        let hits = {
            let picker = picker.clone();
            let state = state.clone();
            let rects = rects.clone();
            let disabled = self.disabled;
            canvas(
                move |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                move |bounds, hitbox, window, cx| {
                    if full {
                        let w = f32::from(bounds.size.width);
                        if (state.read(cx).track_w - w).abs() > 0.5 {
                            state.update(cx, |s, _| s.track_w = w);
                            window.request_animation_frame();
                        }
                    }
                    if disabled {
                        return;
                    }
                    let origin = f32::from(bounds.origin.x);
                    {
                        let (picker, state, rects) = (picker.clone(), state.clone(), rects.clone());
                        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || e.button != MouseButton::Left
                                || !hitbox.is_hovered(window)
                            {
                                return;
                            }
                            let x = f32::from(e.position.x) - origin;
                            let Some(ix) = option_at(&rects, x).filter(|&i| picker.enabled[i])
                            else {
                                return;
                            };
                            state.update(cx, |s, cx| {
                                s.press = Some(Press { x, moved: false });
                                s.keyboard = false;
                                cx.notify();
                            });
                            picker.pick(ix, window, cx);
                        });
                    }
                    if !pressed {
                        return;
                    }
                    {
                        let (picker, state, rects) = (picker.clone(), state.clone(), rects.clone());
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            let x = f32::from(e.position.x) - origin;
                            let held = e.pressed_button == Some(MouseButton::Left);
                            let drag = state.update(cx, |s, cx| {
                                let Some(press) = s.press.as_mut() else {
                                    return false;
                                };
                                if !held {
                                    s.press = None;
                                    cx.notify();
                                    return false;
                                }
                                if !press.moved && (x - press.x).abs() < 3. {
                                    return false;
                                }
                                press.moved = true;
                                true
                            });
                            if drag && let Some(ix) = option_at(&rects, x) {
                                picker.pick(ix, window, cx);
                            }
                        });
                    }
                    window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble && e.button == MouseButton::Left {
                            state.update(cx, |s, cx| {
                                s.press = None;
                                cx.notify();
                            });
                        }
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        let on_key = {
            let picker = picker.clone();
            let state = state.clone();
            let kind = self.kind;
            move |e: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if e.keystroke.modifiers.modified() {
                    return;
                }
                if picker.key(e.keystroke.key.as_str(), kind, window, cx) {
                    state.update(cx, |s, cx| {
                        s.keyboard = true;
                        cx.notify();
                    });
                    cx.stop_propagation();
                } else if e.keystroke.key == "tab" {
                    state.update(cx, |s, _| s.keyboard = true);
                }
            }
        };

        let track = div()
            .id("track")
            .relative()
            .flex()
            .when(full, |d| d.w_full())
            .gap(px(gap))
            .children(pill)
            .children(rows)
            .children(ring)
            .child(hits)
            .on_key_down(on_key);

        let root = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .when(full, |d| d.w_full())
            .when(!full, |d| d.flex_none())
            .font_family(SANS)
            .text_size(px(m.font))
            .line_height(px(m.line))
            .font_weight(m.label_weight)
            .text_color(p.rest)
            .when(tray, |d| d.p(px(m.pad)).rounded(px(m.radius)).bg(p.track))
            .when(underline, |d| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(m.hairline))
                        .bg(p.line),
                )
            })
            .when(self.disabled, |d| d.opacity(0.5))
            .child(track);

        motion::keep_alive(window);
        root
    }
}

/// Icon, label, count and badge.
fn option_content(
    o: &SegmentOption,
    look: Look,
    m: &Metrics,
    p: &Palette,
    active: bool,
    a: f32,
    icon_color: Hsla,
) -> impl IntoElement {
    let label = o.label.clone().map(|label| match look {
        Look::Clean => div().child(label).into_any_element(),
        // Regular and semibold copies crossfade; the semibold one reserves the width
        Look::Classic => div()
            .relative()
            .flex_none()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .opacity(a)
                    .child(label.clone()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .justify_center()
                    .opacity(1. - a)
                    .child(label),
            )
            .into_any_element(),
    });

    let count = o.count.clone().map(|count| {
        let clean_active = look == Look::Clean && active;
        let chip = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .min_w(px(m.count_min))
            .h(px(m.count_min))
            .px(px(m.count_pad))
            .rounded(px(m.count_radius))
            .text_size(px(m.count_font))
            .line_height(px(m.count_line))
            .font_weight(m.count_weight)
            .bg(if clean_active { p.pill } else { p.count_bg })
            .text_color(if clean_active { p.ink } else { p.count_fg })
            .when(clean_active, |d| d.shadow(vec![ring(0.5, p.line)]))
            .child(count.clone());
        let spec = MotionSpec::new(m.count_in, m.curve);
        // A new count replays the entrance
        motion::rise_in(
            gpui::ElementId::Name(format!("count-{count}").into()),
            spec,
            m.count_rise,
            chip,
        )
    });

    let badge = o.badge.clone().map(|badge| {
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(m.badge_h))
            .px(px(m.badge_pad))
            .rounded(px(m.badge_radius))
            .bg(p.accent_soft)
            .text_size(px(m.badge_font))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.accent)
            .child(badge)
    });

    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(m.content_gap))
        .when_some(o.icon.clone(), |d, path| {
            d.child(
                svg()
                    .path(path)
                    .size(px(m.icon))
                    .flex_none()
                    .text_color(icon_color),
            )
        })
        .children(label)
        .children(count)
        .children(badge)
}

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

fn demo_icon(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/segmented-control/clean/{name}.svg").into(),
        Look::Classic => format!("icons/segmented-control/{name}.svg").into(),
    }
}

/// The dashboard picks these in turn after load, until someone interacts.
const CYCLE: [&str; 4] = ["analytics", "reports", "notifications", "overview"];

struct DemoState {
    section: SharedString,
    layout: SharedString,
    billing: SharedString,
    cycle: Option<Task<()>>,
}

impl DemoState {
    fn new(cx: &mut gpui::Context<Self>) -> Self {
        let cycle = cx.spawn(async move |this, cx| {
            for (n, value) in CYCLE.iter().enumerate() {
                let wait = if n == 0 { 1000 } else { 1400 };
                cx.background_executor()
                    .timer(Duration::from_millis(wait))
                    .await;
                let updated = this.update(cx, |s, cx| {
                    s.section = (*value).into();
                    cx.notify();
                });
                if updated.is_err() {
                    break;
                }
            }
        });
        Self {
            section: "overview".into(),
            layout: "board".into(),
            billing: "yearly".into(),
            cycle: Some(cycle),
        }
    }
}

#[derive(IntoElement)]
struct Demo;

impl RenderOnce for Demo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = cx.look();
        let dark = cx.is_dark();
        let clean = look == Look::Clean;
        let state =
            window.use_keyed_state("segmented-control-demo", cx, |_, cx| DemoState::new(cx));
        let s = state.read(cx);
        let (section, layout, billing) = (s.section.clone(), s.layout.clone(), s.billing.clone());

        let (card, shadows, line, label) = match (look, dark) {
            (Look::Clean, false) => (
                hex(0xffffff),
                vec![
                    ring(0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                black_a(0.06),
                black_a(0.45),
            ),
            (Look::Clean, true) => (
                hex(0x1c1c1e),
                vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                white_a(0.08),
                white_a(0.46),
            ),
            (Look::Classic, false) => (
                hex(0xffffff),
                vec![
                    ring(1., hex(0xececec)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ],
                hex(0xececec),
                hex(0x8f8f8f),
            ),
            (Look::Classic, true) => (
                hex(0x19191b),
                vec![ring(1., hex(0x2a2a2e))],
                hex(0x2a2a2e),
                hex(0x8f8f8f),
            ),
        };

        // Any press or key stops the automatic tour
        let stop_mouse = {
            let state = state.clone();
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| s.cycle = None);
            }
        };
        let stop_key = {
            let state = state.clone();
            move |_: &KeyDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| s.cycle = None);
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

        let row = |ix: usize, title: &'static str, control: AnyElement| {
            div()
                .flex()
                .flex_col()
                .items_start()
                .min_w_0()
                .gap(px(if clean { 8. } else { 10. }))
                .py(px(if clean { 16. } else { 18. }))
                .when(ix > 0, |d| {
                    d.border_t(px(if clean { 0.5 } else { 1. }))
                        .border_color(line)
                })
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(if clean { 16. } else { 18. }))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(label)
                        .child(title),
                )
                .child(control)
        };

        let sections = vec![
            SegmentOption::new("overview", "Overview"),
            SegmentOption::new("analytics", "Analytics"),
            SegmentOption::new("reports", "Reports"),
            SegmentOption::new("notifications", "Notifications").count(3),
        ];
        let layouts = vec![
            SegmentOption::new("list", "List").icon(demo_icon(look, "list")),
            SegmentOption::new("board", "Board").icon(demo_icon(look, "board")),
            SegmentOption::new("calendar", "Calendar").icon(demo_icon(look, "calendar")),
        ];
        let plans = vec![
            SegmentOption::new("monthly", "Monthly"),
            SegmentOption::new("yearly", "Yearly").badge("Save 20%"),
        ];

        div()
            .id("seg-demo")
            .flex()
            .flex_col()
            .w(px(440.))
            .px(px(20.))
            .py(px(4.))
            .bg(card)
            .rounded(px(if clean { 10. } else { 16. }))
            .shadow(shadows)
            .capture_any_mouse_down(stop_mouse)
            .capture_key_down(stop_key)
            .child(row(
                0,
                "Dashboard",
                SegmentedControl::new("section", sections)
                    .value(section)
                    .on_change(setter(|s| &mut s.section))
                    .into_any_element(),
            ))
            .child(row(
                1,
                "Layout",
                SegmentedControl::new("layout", layouts)
                    .value(layout)
                    .size(SegmentSize::Sm)
                    .variant(SegmentVariant::Tray)
                    .on_change(setter(|s| &mut s.layout))
                    .into_any_element(),
            ))
            .child(row(
                2,
                "Billing",
                SegmentedControl::new("billing", plans)
                    .value(billing)
                    .variant(SegmentVariant::Underline)
                    .on_change(setter(|s| &mut s.billing))
                    .into_any_element(),
            ))
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| Demo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_option() {
        let rects = [(0., 50.), (58., 40.), (106., 60.)];
        assert_eq!(option_at(&rects, -10.), Some(0));
        assert_eq!(option_at(&rects, 20.), Some(0));
        assert_eq!(option_at(&rects, 53.), Some(0));
        assert_eq!(option_at(&rects, 56.), Some(1));
        assert_eq!(option_at(&rects, 300.), Some(2));
        assert_eq!(option_at(&[], 3.), None);
    }
}
