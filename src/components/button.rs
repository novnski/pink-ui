//! Button – primary, secondary, ghost and destructive buttons with icons,
//! sizes and a shimmering loading state.
//!
//! Clean: a dark chip, a white button with hairline shadows, a quiet ghost and
//! a minimal red fill, 24/28/32 high. Classic: a dark gradient with a light
//! edge, a white outlined button, a gray ghost layer and a light red fill,
//! 32/40/48 high. Both invert in dark mode.
//!
//! When the label or an icon changes, the width glides to its new size and the
//! new part fades in. While loading, a band of light sweeps across the dimmed
//! label, a spinner takes the place of the leading icon and clicks are
//! ignored. Pressing shrinks the button a little; Enter and Space click it
//! when it has focus, Tab moves on to the next one.
//!
//! ```ignore
//! Button::new("save").label("Save changes").on_click(|_, _, _| {});
//! Button::new("upload")
//!     .variant(ButtonVariant::Secondary)
//!     .icon(button::icon(cx.look(), "upload"))
//!     .label("Upload file")
//!     .loading(uploading);
//! ```

use std::{collections::HashMap, f32::consts::TAU, ops::Range, rc::Rc, time::Duration};

use gpui::{
    AnyElement, AnyView, App, BoxShadow, ClickEvent, Context, CursorStyle, Div, ElementId, Entity,
    FocusHandle, FontWeight, Hsla, IntoElement, MouseButton, Pixels, Rgba, SharedString,
    StyledText, Task, TextRun, Transformation, Window, div, font, linear_color_stop,
    linear_gradient, prelude::*, px, radians, svg,
};

use crate::{
    ActiveTheme, Look,
    motion::{self, AnimationExt, CubicBezier, EASE_OUT_QUINT, MotionSpec},
    primitives::{self, SANS, black_a, hex, hexa, white_a},
    story,
};

pub const NAME: &str = "Button";
pub const DESCRIPTION: &str = "Primary, secondary, ghost and destructive buttons with icons, sizes and a shimmering loading state";

/// Drawn in place of the leading icon while loading.
const SPINNER: &str = "icons/button/spinner.svg";

/// `cubic-bezier(0.25, 1, 0.5, 1)`: the morph curve.
const MORPH_CURVE: CubicBezier = CubicBezier::new(0.25, 1.0, 0.5, 1.0);
/// The width glides to its new size.
const MORPH: MotionSpec = MotionSpec::new(480, MORPH_CURVE);
/// A changed label rises in (`ui-button-label-in`).
const LABEL_IN: MotionSpec = MotionSpec::new(480, MORPH_CURVE);
/// A changed icon grows in (`ui-button-icon-in`).
const ICON_IN: MotionSpec = MotionSpec::new(520, MORPH_CURVE);
/// `:active { transform: scale(0.97) }` over `160ms var(--ease)`.
const PRESS: MotionSpec = MotionSpec::new(160, EASE_OUT_QUINT);
/// One sweep of the loading shine: `var(--load) * 1.6`.
const SWEEP: Duration = Duration::from_millis(1040);
const SPIN: Duration = Duration::from_millis(900);

/// The icons from the website demo, drawn for `look` (Clean strokes are a
/// little thinner): `arrow-up`, `arrow-right`, `plus`, `dots`, `copy`,
/// `upload`, `trash` and `check`.
pub fn icon(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/button/clean/{name}.svg").into(),
        Look::Classic => format!("icons/button/{name}.svg").into(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// Clean: a dark chip. Classic: a dark gradient with a light edge.
    #[default]
    Primary,
    /// White with a hairline shadow (Clean) or an outline (Classic).
    Secondary,
    /// A muted label; a soft wash appears on hover.
    Ghost,
    /// A red label on a light red fill, for deleting.
    Destructive,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonSize {
    /// Clean 24, Classic 32 high.
    Sm,
    /// Clean 28, Classic 40 high.
    #[default]
    Md,
    /// Clean 32, Classic 48 high.
    Lg,
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    variant: ButtonVariant,
    size: ButtonSize,
    icon: Option<SharedString>,
    icon_right: Option<SharedString>,
    loading: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
    look: Option<Look>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            icon: None,
            icon_right: None,
            loading: false,
            disabled: false,
            on_click: None,
            look: None,
        }
    }

    /// The text; leave it out for an icon-only square button.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn destructive(self) -> Self {
        self.variant(ButtonVariant::Destructive)
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// An SVG under `assets/` before the label, tinted with the label color
    /// (see [`icon`] for the demo's set).
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// An SVG under `assets/` after the label.
    pub fn icon_right(mut self, path: impl Into<SharedString>) -> Self {
        self.icon_right = Some(path.into());
        self
    }

    /// Shimmers the label, swaps the leading icon for a spinner and ignores clicks.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Fades the button to 40% and turns off hover, press and clicks.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Overrides the global Clean/Classic look for this button.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }

    /// The width the button takes at rest – handy for reserving room for the
    /// widest of several labels, like the website demo's slots do.
    pub fn measure(&self, window: &Window, cx: &App) -> Pixels {
        let look = self.look.unwrap_or(cx.look());
        let m = Metrics::get(look, self.size);
        let label_width = self
            .label
            .as_ref()
            .map_or(0., |label| f32::from(shape(window, label, m.fs).width));
        px(self.natural_width(&m, label_width))
    }

    fn icon_only(&self) -> bool {
        self.label.is_none() && (self.icon.is_some() || self.icon_right.is_some())
    }

    /// Left and right padding: an icon sits closer to the edge than text (Clean).
    fn padding(&self, m: &Metrics) -> (f32, f32) {
        if self.icon_only() {
            return (0., 0.);
        }
        let left = m.pad
            - if self.icon.is_some() {
                m.icon_inset
            } else {
                0.
            };
        let right = m.pad
            - if self.icon_right.is_some() {
                m.icon_inset
            } else {
                0.
            };
        (left, right)
    }

    fn natural_width(&self, m: &Metrics, label_width: f32) -> f32 {
        if self.icon_only() {
            return m.h;
        }
        let (left, right) = self.padding(m);
        let parts = [
            self.icon.is_some(),
            self.label.is_some(),
            self.icon_right.is_some(),
        ];
        let count = parts.iter().filter(|p| **p).count();
        let icons = (self.icon.is_some() as u8 + self.icon_right.is_some() as u8) as f32;
        2. * m.border
            + left
            + right
            + icons * m.icon
            + label_width
            + count.saturating_sub(1) as f32 * m.gap
    }
}

/// A run of the label's font (Inter Medium) in `color`.
fn label_run(len: usize, color: Hsla) -> TextRun {
    TextRun {
        len,
        font: gpui::Font {
            weight: FontWeight::MEDIUM,
            ..font(SANS)
        },
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

/// Shapes `text` the way the label renders: Inter Medium at `size`.
fn shape(window: &Window, text: &SharedString, size: f32) -> gpui::ShapedLine {
    let run = label_run(text.len(), black_a(1.));
    window
        .text_system()
        .shape_line(text.clone(), px(size), &[run], None)
}

/// `--h`, `--px`, `--gap`, `--fs`, `--lh`, `--icon` and `--radius` per look × size.
#[derive(Clone, Copy)]
struct Metrics {
    h: f32,
    pad: f32,
    gap: f32,
    fs: f32,
    lh: f32,
    icon: f32,
    radius: f32,
    /// Classic keeps a 1px (often transparent) border inside its height.
    border: f32,
    /// Clean pulls the padding in by 2px next to an icon.
    icon_inset: f32,
}

impl Metrics {
    fn get(look: Look, size: ButtonSize) -> Self {
        match (look, size) {
            (Look::Clean, ButtonSize::Sm) => Metrics::clean(24., 8., 4., 12., 16., 14.),
            (Look::Clean, ButtonSize::Md) => Metrics::clean(28., 10., 6., 13., 20., 16.),
            (Look::Clean, ButtonSize::Lg) => Metrics::clean(32., 12., 6., 13., 20., 16.),
            (Look::Classic, ButtonSize::Sm) => Metrics::classic(32., 12., 6., 13., 16., 10.),
            (Look::Classic, ButtonSize::Md) => Metrics::classic(40., 16., 8., 14., 18., 12.),
            (Look::Classic, ButtonSize::Lg) => Metrics::classic(48., 20., 10., 15., 20., 14.),
        }
    }

    fn clean(h: f32, pad: f32, gap: f32, fs: f32, lh: f32, icon: f32) -> Self {
        Metrics {
            h,
            pad,
            gap,
            fs,
            lh,
            icon,
            radius: 6.,
            border: 0.,
            icon_inset: 2.,
        }
    }

    /// Classic sets `line-height: 1`.
    fn classic(h: f32, pad: f32, gap: f32, fs: f32, icon: f32, radius: f32) -> Self {
        Metrics {
            h,
            pad,
            gap,
            fs,
            lh: fs,
            icon,
            radius,
            border: 1.,
            icon_inset: 0.,
        }
    }
}

#[derive(Clone, Copy)]
enum Paint {
    Solid(Hsla),
    /// Top to bottom.
    Gradient(Hsla, Hsla),
}

/// One variant's colors in one look × mode (the `--ui-btn-*` / `--ui-btnc-*` tokens).
struct Skin {
    fill: Paint,
    fill_hover: Paint,
    /// Classic primary: the light edge running down the 1px border.
    edge: Option<(Hsla, Hsla)>,
    /// Classic draws a 1px border on every variant, transparent but on secondary.
    border: Option<Hsla>,
    fg: Hsla,
    fg_hover: Hsla,
    icon: Hsla,
    icon_hover: Hsla,
    shadow: Vec<BoxShadow>,
    shadow_hover: Vec<BoxShadow>,
    /// The band of light that sweeps the label while loading.
    shine: Hsla,
    /// The share of the label color kept while loading.
    dim: f32,
    focus: Hsla,
    focus_offset: f32,
}

impl Skin {
    fn get(look: Look, variant: ButtonVariant, dark: bool) -> Self {
        match look {
            Look::Clean => Self::clean(variant, dark),
            Look::Classic => Self::classic(variant, dark),
        }
    }

    fn clean(variant: ButtonVariant, dark: bool) -> Self {
        let (title, ink, muted, icon, solid, solid_hover, on_solid, surface) = if dark {
            (
                white_a(0.9),
                white_a(0.9),
                white_a(0.62),
                white_a(0.5),
                white_a(0.9),
                white_a(0.82),
                hex(0x161618),
                hex(0x1c1c1e),
            )
        } else {
            (
                hex(0x000000),
                black_a(0.88),
                black_a(0.61),
                black_a(0.447),
                black_a(0.88),
                black_a(0.8),
                hex(0xfcfcfc),
                hex(0xffffff),
            )
        };
        let (shadow_rest, shadow_hover) = if dark {
            (
                vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                vec![
                    ring(0.5, white_a(0.16)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
            )
        } else {
            (
                vec![
                    ring(0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                vec![
                    ring(0.5, black_a(0.1)),
                    shadow(0., 1., 1., -1., black_a(0.12)),
                    shadow(0., 1., 3., 0., black_a(0.07)),
                ],
            )
        };
        let wash = if dark { white_a(0.06) } else { black_a(0.03) };
        let (danger, danger_bg, danger_bg_hover) = if dark {
            (hex(0xff6369), hexa(0xe5484d, 0.14), hexa(0xe5484d, 0.2))
        } else {
            (hex(0xe5484d), hexa(0xe5484d, 0.08), hexa(0xe5484d, 0.12))
        };
        let shine = if dark { hex(0xffffff) } else { white_a(0.82) };
        let focus = if dark {
            hexa(0x4d8dff, 0.5)
        } else {
            hexa(0x0060e2, 0.35)
        };
        let base = Skin {
            fill: Paint::Solid(solid),
            fill_hover: Paint::Solid(solid_hover),
            edge: None,
            border: None,
            fg: on_solid,
            fg_hover: on_solid,
            icon: on_solid,
            icon_hover: on_solid,
            shadow: Vec::new(),
            shadow_hover: Vec::new(),
            // `--ui-btnc-shine-on-solid` is white in both modes
            shine: hex(0xffffff),
            dim: 0.5,
            focus,
            focus_offset: 1.,
        };
        match variant {
            ButtonVariant::Primary => base,
            ButtonVariant::Secondary => Skin {
                fill: Paint::Solid(surface),
                fill_hover: Paint::Solid(surface),
                fg: title,
                fg_hover: title,
                icon,
                icon_hover: icon,
                shadow: shadow_rest,
                shadow_hover,
                shine,
                ..base
            },
            ButtonVariant::Ghost => Skin {
                fill: Paint::Solid(wash.opacity(0.)),
                fill_hover: Paint::Solid(wash),
                fg: muted,
                fg_hover: ink,
                icon,
                icon_hover: ink,
                shine,
                ..base
            },
            ButtonVariant::Destructive => Skin {
                fill: Paint::Solid(danger_bg),
                fill_hover: Paint::Solid(danger_bg_hover),
                fg: danger,
                fg_hover: danger,
                icon: danger,
                icon_hover: danger,
                shine,
                ..base
            },
        }
    }

    fn classic(variant: ButtonVariant, dark: bool) -> Self {
        let focus = if dark {
            hexa(0xededed, 0.3)
        } else {
            hexa(0x171717, 0.22)
        };
        let shine = if dark {
            hexa(0xededed, 0.95)
        } else {
            white_a(0.82)
        };
        let base = Skin {
            fill: Paint::Solid(hex(0x000000).opacity(0.)),
            fill_hover: Paint::Solid(hex(0x000000).opacity(0.)),
            edge: None,
            border: Some(hex(0x000000).opacity(0.)),
            fg: hex(0x171717),
            fg_hover: hex(0x171717),
            icon: hex(0x171717),
            icon_hover: hex(0x171717),
            shadow: Vec::new(),
            shadow_hover: Vec::new(),
            shine,
            dim: 0.55,
            focus,
            focus_offset: 2.,
        };
        match (variant, dark) {
            (ButtonVariant::Primary, false) => {
                let fg = hex(0xfafafa);
                let shadows = vec![
                    ring(1., hex(0x0d0d0d)),
                    shadow(0., 2., 4., 0., black_a(0.1)),
                ];
                Skin {
                    fill: Paint::Gradient(hex(0x323137), hex(0x201e25)),
                    fill_hover: Paint::Gradient(hex(0x3c3b42), hex(0x29272f)),
                    edge: Some((hex(0x4b4951), hex(0x18171b))),
                    fg,
                    fg_hover: fg,
                    icon: fg,
                    icon_hover: fg,
                    shadow: shadows.clone(),
                    shadow_hover: shadows,
                    shine: white_a(0.95),
                    ..base
                }
            }
            (ButtonVariant::Primary, true) => {
                let fg = hex(0x121213);
                let shadows = vec![ring(1., black_a(0.6)), shadow(0., 2., 4., 0., black_a(0.4))];
                Skin {
                    fill: Paint::Gradient(hex(0xf4f4f5), hex(0xe4e4e7)),
                    fill_hover: Paint::Gradient(hex(0xffffff), hex(0xececef)),
                    edge: Some((hex(0xffffff), hex(0xd4d4d8))),
                    fg,
                    fg_hover: fg,
                    icon: fg,
                    icon_hover: fg,
                    shadow: shadows.clone(),
                    shadow_hover: shadows,
                    shine: white_a(0.9),
                    ..base
                }
            }
            (ButtonVariant::Secondary, _) => {
                let (surface, hover, line, ink, lift) = if dark {
                    (
                        hex(0x232326),
                        hex(0x2a2a2d),
                        white_a(0.1),
                        hex(0xededed),
                        black_a(0.4),
                    )
                } else {
                    (
                        hex(0xffffff),
                        hex(0xfafafa),
                        hex(0xe5e5e5),
                        hex(0x171717),
                        black_a(0.06),
                    )
                };
                let shadows = vec![shadow(0., 1., 2., 0., lift)];
                Skin {
                    fill: Paint::Solid(surface),
                    fill_hover: Paint::Solid(hover),
                    border: Some(line),
                    fg: ink,
                    fg_hover: ink,
                    icon: ink,
                    icon_hover: ink,
                    shadow: shadows.clone(),
                    shadow_hover: shadows,
                    ..base
                }
            }
            (ButtonVariant::Ghost, _) => {
                let (muted, hover, layer) = if dark {
                    (hex(0x8f8f8f), hex(0xededed), white_a(0.06))
                } else {
                    (hex(0x737373), hex(0x404040), hex(0xf0f0f0))
                };
                Skin {
                    fill: Paint::Solid(layer.opacity(0.)),
                    fill_hover: Paint::Solid(layer),
                    fg: muted,
                    fg_hover: hover,
                    icon: muted,
                    icon_hover: hover,
                    ..base
                }
            }
            (ButtonVariant::Destructive, _) => {
                let (danger, danger_bg) = if dark {
                    (hex(0xff6b6b), hexa(0xff4040, 0.16))
                } else {
                    (hex(0xff0000), hex(0xffe6e9))
                };
                Skin {
                    fill: Paint::Solid(danger_bg),
                    // color-mix(in srgb, bg 88%, danger)
                    fill_hover: Paint::Solid(motion::mix(danger_bg, danger, 0.12)),
                    fg: danger,
                    fg_hover: danger,
                    icon: danger,
                    icon_hover: danger,
                    shine: white_a(0.82),
                    ..base
                }
            }
        }
    }
}

impl Paint {
    fn mix(self, hover: Paint, t: f32) -> Paint {
        match (self, hover) {
            (Paint::Solid(a), Paint::Solid(b)) => Paint::Solid(motion::mix(a, b, t)),
            (Paint::Gradient(a, b), Paint::Gradient(c, d)) => {
                Paint::Gradient(motion::mix(a, c, t), motion::mix(b, d, t))
            }
            (rest, hover) => {
                if t < 0.5 {
                    rest
                } else {
                    hover
                }
            }
        }
    }
}

fn gradient(top: Hsla, bottom: Hsla) -> gpui::Background {
    linear_gradient(
        180.,
        linear_color_stop(top, 0.),
        linear_color_stop(bottom, 1.),
    )
}

/// Eases each shadow layer toward its hover twin, like `transition: box-shadow`.
fn mix_shadows(rest: &[BoxShadow], hover: &[BoxShadow], t: f32) -> Vec<BoxShadow> {
    if rest.len() != hover.len() {
        return if t < 0.5 {
            rest.to_vec()
        } else {
            hover.to_vec()
        };
    }
    rest.iter()
        .zip(hover)
        .map(|(a, b)| BoxShadow {
            color: motion::mix(a.color, b.color, t),
            offset: gpui::point(
                px(motion::lerp(
                    f32::from(a.offset.x),
                    f32::from(b.offset.x),
                    t,
                )),
                px(motion::lerp(
                    f32::from(a.offset.y),
                    f32::from(b.offset.y),
                    t,
                )),
            ),
            blur_radius: px(motion::lerp(
                f32::from(a.blur_radius),
                f32::from(b.blur_radius),
                t,
            )),
            spread_radius: px(motion::lerp(
                f32::from(a.spread_radius),
                f32::from(b.spread_radius),
                t,
            )),
        })
        .collect()
}

/// `box-shadow: x y blur spread color`. GPUI's blur radius is the Gaussian
/// sigma, which CSS sets to half the blur, and a zero blur paints nothing, so
/// crisp rings keep a hair of blur to stay visible.
fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    primitives::shadow(x, y, (blur / 2.).max(0.3), spread, color)
}

/// `box-shadow: 0 0 0 <width> color`.
fn ring(width: f32, color: Hsla) -> BoxShadow {
    shadow(0., 0., 0., width, color)
}

/// `top` painted over `bottom` (source-over).
fn over(top: Hsla, bottom: Hsla) -> Hsla {
    let (t, b) = (Rgba::from(top), Rgba::from(bottom));
    let a = t.a + b.a * (1. - t.a);
    if a <= f32::EPSILON {
        return top.opacity(0.);
    }
    let channel = |x: f32, y: f32| (x * t.a + y * b.a * (1. - t.a)) / a;
    Hsla::from(Rgba {
        r: channel(t.r, b.r),
        g: channel(t.g, b.g),
        b: channel(t.b, b.b),
        a,
    })
}

/// What the web version keys its parts by: a change remounts the part, which
/// replays its entrance, and the width glides.
#[derive(Clone, PartialEq)]
struct Content {
    label: Option<SharedString>,
    icon: Option<SharedString>,
    icon_right: Option<SharedString>,
}

struct ButtonState {
    focus: FocusHandle,
    pressed: bool,
    /// Focus that came from a click shows no ring (`:focus-visible`).
    pointer_focus: bool,
    content: Option<Content>,
    /// Bumped when the label, the leading icon or the trailing icon changes.
    generation: [u64; 3],
}

impl ButtonState {
    fn sync(&mut self, content: Content, focused: bool) -> ([u64; 3], bool, bool) {
        if let Some(prev) = &self.content {
            if prev.label != content.label {
                self.generation[0] += 1;
            }
            if prev.icon != content.icon {
                self.generation[1] += 1;
            }
            if prev.icon_right != content.icon_right {
                self.generation[2] += 1;
            }
        }
        self.content = Some(content);
        if !focused {
            self.pointer_focus = false;
        }
        (self.generation, self.pressed, self.pointer_focus)
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let skin = Skin::get(look, self.variant, cx.is_dark());
        let m = Metrics::get(look, self.size);
        let key = format!("button-{}", self.id);
        let reduced = motion::reduced_motion(cx);

        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "button".into()),
            cx,
            |_, cx| ButtonState {
                focus: cx.focus_handle().tab_stop(true),
                pressed: false,
                pointer_focus: false,
                content: None,
                generation: [0; 3],
            },
        );
        let focus = state.read(cx).focus.clone();
        let focused = !self.disabled && focus.is_focused(window);
        let content = Content {
            label: self.label.clone(),
            icon: match (&self.icon, self.loading) {
                (Some(_), true) => Some(SPINNER.into()),
                (icon, _) => icon.clone(),
            },
            icon_right: self.icon_right.clone(),
        };
        let (generation, pressed, pointer_focus) =
            state.update(cx, |s, _| s.sync(content, focused));

        // Hover still shows while loading; disabled buttons stay put.
        let t = if self.disabled {
            0.
        } else {
            motion::hover_t(&key)
        };
        let pressable = !self.disabled && !self.loading;
        let fg = motion::mix(skin.fg, skin.fg_hover, t);
        let icon_color = motion::mix(skin.icon, skin.icon_hover, t);

        // The width glides when the content changes; pressing scales it all by 0.97.
        let label_shape = self.label.as_ref().map(|label| shape(window, label, m.fs));
        let label_width = label_shape.as_ref().map_or(0., |s| f32::from(s.width));
        let natural = self.natural_width(&m, label_width);
        let width = motion::follow(
            &format!("{key}-width-{look:?}-{:?}", self.size),
            natural,
            MORPH,
        );
        let scale = motion::follow(
            &format!("{key}-press"),
            if pressed && pressable { 0.97 } else { 1. },
            PRESS,
        );
        let morphing = (width - natural).abs() > 0.01;
        let pressing = scale < 0.9999;
        let s = |v: f32| px(v * scale);
        let (pad_left, pad_right) = self.padding(&m);
        let radius = m.radius * scale;

        let mut root = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(s(m.gap))
            .h(s(m.h))
            .pl(s(pad_left))
            .pr(s(pad_right))
            .rounded(px(radius))
            .font_family(SANS)
            .font_weight(FontWeight::MEDIUM)
            .text_size(s(m.fs))
            .line_height(s(m.lh))
            .whitespace_nowrap()
            .text_color(fg)
            // GPUI fades each layer on its own, so a shadow would show
            // through a disabled button's faded fill: leave it out.
            .when(!self.disabled, |d| {
                d.shadow(mix_shadows(&skin.shadow, &skin.shadow_hover, t))
            })
            .cursor(if pressable {
                CursorStyle::PointingHand
            } else {
                CursorStyle::Arrow
            })
            .when(self.disabled, |d| d.opacity(0.4));

        // Fill, or for the Classic primary the edge gradient under a fill layer
        let fill = skin.fill.mix(skin.fill_hover, t);
        root = match (skin.edge, fill) {
            (Some((top, bottom)), _) => root.bg(gradient(top, bottom)),
            (None, Paint::Solid(color)) => root.bg(color),
            (None, Paint::Gradient(top, bottom)) => root.bg(gradient(top, bottom)),
        };
        if let Some(border) = skin.border {
            root = root.border(px(m.border)).border_color(border);
        }
        if self.icon_only() {
            root = root.w(s(m.h));
        } else if morphing || pressing {
            root = root.w(s(width)).overflow_hidden();
        }
        if pressing {
            let base_w = if self.icon_only() { m.h } else { width };
            root = root
                .mx(px(base_w * (1. - scale) / 2.))
                .my(px(m.h * (1. - scale) / 2.));
        }

        if !self.disabled {
            root = root
                .track_focus(&focus)
                .on_hover(motion::hover_listener(key.clone()))
                .on_key_down(|event, window, cx| {
                    if event.keystroke.key == "tab" {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev();
                        } else {
                            window.focus_next();
                        }
                        cx.stop_propagation();
                    }
                })
                .on_mouse_down(MouseButton::Left, {
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.pressed = true;
                            s.pointer_focus = true;
                            cx.notify();
                        })
                    }
                });
            let release = {
                let state = state.clone();
                move |_: &gpui::MouseUpEvent, _: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| {
                        if s.pressed {
                            s.pressed = false;
                            cx.notify();
                        }
                    })
                }
            };
            root = root
                .on_mouse_up(MouseButton::Left, release.clone())
                .on_mouse_up_out(MouseButton::Left, release);
            if pressable && let Some(handler) = self.on_click.clone() {
                root = root.on_click(move |event, window, cx| handler(event, window, cx));
            }
        }

        if let (Some(_), Paint::Gradient(top, bottom)) = (skin.edge, fill) {
            // The padding-box fill inside the 1px light edge
            root = root.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px((radius - m.border).max(0.)))
                    .bg(gradient(top, bottom)),
            );
        }

        let icon_size = m.icon * scale;
        if let Some(path) = self.icon.clone() {
            let path = if self.loading { SPINNER.into() } else { path };
            root = root.child(icon_slot(
                &key,
                "icon",
                path,
                icon_size,
                icon_color,
                generation[1],
                self.loading,
                reduced,
            ));
        }
        if let (Some(label), Some(shape)) = (self.label.clone(), label_shape) {
            let sweep = (self.loading && !reduced).then(|| Sweep {
                chars: label
                    .char_indices()
                    .map(|(ix, ch)| {
                        let end = ix + ch.len_utf8();
                        let x0 = f32::from(shape.x_for_index(ix));
                        let x1 = f32::from(shape.x_for_index(end));
                        (ix..end, (x0 + x1) / 2. * scale)
                    })
                    .collect(),
                width: label_width * scale,
                shine: skin.shine,
            });
            root = root.child(label_element(
                &key,
                label,
                fg,
                if self.loading { skin.dim } else { 1. },
                m.lh * scale,
                generation[0],
                sweep,
                reduced,
            ));
        }
        if let Some(path) = self.icon_right.clone() {
            root = root.child(icon_slot(
                &key,
                "icon-right",
                path,
                icon_size,
                icon_color,
                generation[2],
                false,
                reduced,
            ));
        }

        if focused && !pointer_focus {
            // `outline: 2px solid; outline-offset`, drawn outside the border
            let out = skin.focus_offset + 2. + m.border;
            root = root.child(
                div()
                    .absolute()
                    .top(px(-out))
                    .left(px(-out))
                    .right(px(-out))
                    .bottom(px(-out))
                    .rounded(px(radius + out))
                    .border_2()
                    .border_color(skin.focus),
            );
        }

        motion::keep_alive(window);
        root
    }
}

/// The 16–20px box an icon (or the spinner) sits in. A new icon grows in from
/// 70% while it fades in.
#[allow(clippy::too_many_arguments)]
fn icon_slot(
    key: &str,
    part: &'static str,
    path: SharedString,
    size: f32,
    color: Hsla,
    generation: u64,
    spinning: bool,
    reduced: bool,
) -> Div {
    let glyph = svg().path(path).size_full().text_color(color);
    let glyph = if spinning {
        glyph
            .with_animation(
                ElementId::Name(format!("{key}-{part}-spin").into()),
                gpui::Animation::new(SPIN).repeat(),
                |svg, t| svg.with_transformation(Transformation::rotate(radians(t * TAU))),
            )
            .into_any_element()
    } else {
        glyph.into_any_element()
    };
    let inner = div().size(px(size)).child(glyph);
    let inner = if generation > 0 && !reduced {
        inner
            .with_animation(
                ElementId::NamedInteger(format!("{key}-{part}-in").into(), generation),
                ICON_IN.animation(),
                move |el, t| el.size(px(size * (0.7 + 0.3 * t))).opacity(t),
            )
            .into_any_element()
    } else {
        inner.into_any_element()
    };
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .child(inner)
}

/// Where each character sits, for the loading shine.
struct Sweep {
    chars: Vec<(Range<usize>, f32)>,
    width: f32,
    shine: Hsla,
}

/// The label; a new one rises in, and while loading a band of light sweeps
/// across a dimmed copy (`background-clip: text` done per character).
#[allow(clippy::too_many_arguments)]
fn label_element(
    key: &str,
    label: SharedString,
    color: Hsla,
    dim: f32,
    line_height: f32,
    generation: u64,
    sweep: Option<Sweep>,
    reduced: bool,
) -> AnyElement {
    let base = color.opacity(dim);
    let text = div().flex_none().text_color(base);
    let text = match sweep {
        Some(sweep) => text
            .with_animation(
                ElementId::Name(format!("{key}-sweep").into()),
                gpui::Animation::new(SWEEP).repeat(),
                move |el, t| {
                    // A 300%-wide gradient sliding from 100% to 0%: the band's
                    // center travels from -½ to 1½ label widths.
                    let center = sweep.width * (2. * t - 0.5);
                    let half = sweep.width * 0.36;
                    // Explicit runs: highlight colors would be blended over
                    // the dimmed label instead of replacing it.
                    let runs = sweep
                        .chars
                        .iter()
                        .map(|(range, x)| {
                            let a = (1. - (x - center).abs() / half).max(0.);
                            label_run(range.len(), over(sweep.shine.opacity(a), base))
                        })
                        .collect();
                    el.child(StyledText::new(label.clone()).with_runs(runs))
                },
            )
            .into_any_element(),
        None => text.child(label).into_any_element(),
    };
    let wrapper = div().flex_none().child(text);
    if generation > 0 && !reduced {
        // translateY(25%) of the label box
        let rise = line_height * 0.25;
        wrapper
            .with_animation(
                ElementId::NamedInteger(format!("{key}-label-in").into(), generation),
                LABEL_IN.animation(),
                move |el, t| el.relative().top(px(rise * (1. - t))).opacity(t),
            )
            .into_any_element()
    } else {
        wrapper.into_any_element()
    }
}

// ---------------------------------------------------------------------------
// Story: the website's showcase
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Upload {
    #[default]
    Idle,
    Loading,
    Done,
}

#[derive(Default)]
struct Demo {
    copied: bool,
    copied_icon: bool,
    upload: Upload,
    tasks: HashMap<&'static str, Task<()>>,
    /// Each button's widest width so far, so a shrinking label never moves
    /// its neighbors (the demo's `button-demo__slot`).
    slots: HashMap<String, Pixels>,
}

impl Demo {
    /// `useFlash(1600)`: on now, off 1.6s after the last click.
    fn flash(&mut self, which: &'static str, cx: &mut Context<Self>) {
        self.set_flag(which, true);
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1600))
                .await;
            this.update(cx, |demo, cx| {
                demo.set_flag(which, false);
                cx.notify();
            })
            .ok();
        });
        self.tasks.insert(which, task);
        cx.notify();
    }

    fn set_flag(&mut self, which: &str, on: bool) {
        match which {
            "copy" => self.copied = on,
            _ => self.copied_icon = on,
        }
    }

    fn upload(&mut self, cx: &mut Context<Self>) {
        if self.upload == Upload::Loading {
            return;
        }
        self.upload = Upload::Loading;
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1800))
                .await;
            this.update(cx, |demo, cx| {
                demo.upload = Upload::Done;
                cx.notify();
            })
            .ok();
        });
        self.tasks.insert("upload", task);
        cx.notify();
    }
}

fn slot(demo: &Entity<Demo>, button: Button, window: &mut Window, cx: &mut App) -> Div {
    let width = button.measure(window, cx);
    let key = format!("{}-{:?}", button.id, cx.look());
    let min = demo.update(cx, |demo, _| {
        let widest = demo.slots.entry(key).or_insert(width);
        if width > *widest {
            *widest = width;
        }
        *widest
    });
    div().flex().justify_center().min_w(min).child(button)
}

fn showcase(window: &mut Window, cx: &mut App) -> Div {
    let look = cx.look();
    let muted = if cx.is_dark() {
        hex(0x8f8f8f)
    } else {
        hex(0x737373)
    };
    let demo = window.use_keyed_state("button-demo", cx, |_, _| Demo::default());
    let (copied, copied_icon, upload) = {
        let d = demo.read(cx);
        (d.copied, d.copied_icon, d.upload)
    };
    let i = |name: &str| icon(look, name);

    let styles = vec![
        Button::new("save").label("Save changes"),
        Button::new("secondary").secondary().label("Secondary"),
        Button::new("ghost").ghost().label("Ghost"),
        Button::new("delete")
            .destructive()
            .icon(i("trash"))
            .label("Delete"),
    ];
    let sizes = vec![
        Button::new("add")
            .secondary()
            .size(ButtonSize::Sm)
            .icon(i("plus"))
            .label("Add item"),
        Button::new("continue")
            .icon_right(i("arrow-right"))
            .label("Continue"),
        Button::new("more").ghost().icon(i("dots")),
        Button::new("large")
            .secondary()
            .size(ButtonSize::Lg)
            .label("Large"),
    ];
    let states = vec![
        Button::new("copy-link")
            .secondary()
            .icon(i(if copied { "check" } else { "copy" }))
            .label(if copied { "Copied" } else { "Copy link" })
            .on_click({
                let demo = demo.clone();
                move |_, _, cx| demo.update(cx, |d, cx| d.flash("copy", cx))
            }),
        Button::new("copy")
            .secondary()
            .icon(i(if copied_icon { "check" } else { "copy" }))
            .on_click({
                let demo = demo.clone();
                move |_, _, cx| demo.update(cx, |d, cx| d.flash("copy-icon", cx))
            }),
        Button::new("upload")
            .secondary()
            .icon(i(if upload == Upload::Done {
                "check"
            } else {
                "upload"
            }))
            .loading(upload == Upload::Loading)
            .label("Upload file")
            .on_click({
                let demo = demo.clone();
                move |_, _, cx| demo.update(cx, |d, cx| d.upload(cx))
            }),
        Button::new("disabled")
            .secondary()
            .disabled(true)
            .label("Disabled"),
    ];

    let mut row = |buttons: Vec<Button>| {
        div()
            .flex()
            .flex_wrap()
            .justify_center()
            .items_center()
            .gap(px(10.))
            .children(
                buttons
                    .into_iter()
                    .map(|button| slot(&demo, button, window, cx)),
            )
    };
    let heading = |title: &'static str| {
        div()
            .text_size(px(13.))
            .line_height(px(18.571))
            .text_color(muted)
            .child(title)
    };
    let section = |title: &'static str, row: Div| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.))
            .child(heading(title))
            .child(row)
    };
    let status = match upload {
        Upload::Idle => "",
        Upload::Loading => "Uploading…",
        Upload::Done => "Upload complete",
    };

    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(22.))
        .child(section("Styles", row(styles)))
        .child(section("Icons and sizes", row(sizes)))
        .child(
            section("Feedback and states", row(states)).child(
                div()
                    .mt(px(4.))
                    .min_h(px(18.571))
                    .text_size(px(13.))
                    .line_height(px(18.571))
                    .text_color(muted)
                    .child(status),
            ),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| showcase(window, cx))
}
