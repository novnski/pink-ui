//! Appearance Picker – Settings › Appearance (Pink: settings/appearance.rs
//! `render` Theme and Text sections, with `option_card`, `accent_swatch`,
//! `render_theme_selector`, `render_font_picker`, `render_size_picker` and
//! `render_width_control`).
//!
//! The Theme card: System, Light and Dark as option cards whose previews are
//! miniature windows drawn in the themes you picked (System splits light
//! and dark), a pop-up menu each for the light and the dark theme with a
//! palette chip per theme, the accent swatches (the theme's own accent and
//! Pink's seven presets, each ringed in its own color when chosen), and the
//! Glass choice. The Text card: the interface font with a filterable family
//! menu and a size ladder that scales the whole interface, and the
//! conversation width slider with its value and a reset. Choosing an
//! accent re-tints the page's own accent parts, as it does in Pink.
//!
//! ```ignore
//! AppearancePicker::new("appearance")
//!     .mode(AppearanceMode::System)
//!     .accent(AccentChoice::Blue)
//!     .on_change(|change, _, _| persist(change))
//! ```

use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

use gpui::{
    Anchor, AnyElement, App, Bounds, ClickEvent, Context, DispatchPhase, Div, ElementId, Entity,
    FocusHandle, Hsla, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Rgba, Role, SharedString, Stateful, Subscription, Task, Toggled, Window,
    anchored, canvas, deferred, div, prelude::*, px, svg,
};

use crate::{
    motion::{self, MENU_OUT, MENU_RISE_DOWN},
    pink::{self, ControlSize, FillTier, Pink, Segment, TextStyle, TypeStyleExt, layout},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Appearance Picker";
pub const DESCRIPTION: &str = "Pink's Appearance settings – System, Light and Dark as miniature windows of the themes you pick, a menu each for the light and dark theme, accent swatches that re-tint the page, frosted or opaque glass, the interface font and size, and a conversation width slider";

const ICON_SYSTEM: &str = "icons/appearance-picker/monitor.svg";
const ICON_LIGHT: &str = "icons/appearance-picker/sun.svg";
const ICON_DARK: &str = "icons/appearance-picker/moon.svg";
const ICON_RESTART: &str = "icons/appearance-picker/restart.svg";

/// Pink's settings column: `max_w 720` less its 32 px gutters.
const CARD_W: f32 = 656.0;

/// Option cards (Pink widgets.rs `OPTION_CARD_HEIGHT`, `OPTION_CARD_RADIUS`,
/// `option_card_row`'s 16 gap).
const OPTION_CARD_HEIGHT: f32 = 148.0;
const OPTION_CARD_RADIUS: f32 = 6.0;
const OPTION_CARD_GAP: f32 = 16.0;

/// Pickers and menus (Pink appearance.rs).
const FAMILY_PICKER_WIDTH: f32 = 200.0;
const FAMILY_MENU_WIDTH: f32 = 240.0;
const FAMILY_MENU_MAX_HEIGHT: f32 = 280.0;
const SIZE_PICKER_WIDTH: f32 = 92.0;
const SIZE_MENU_WIDTH: f32 = 112.0;
const THEME_PICKER_WIDTH: f32 = 220.0;
const THEME_MENU_WIDTH: f32 = 260.0;
const SEGMENTED_GLASS_WIDTH: f32 = 300.0;

/// The conversation width slider (Pink appearance.rs `WIDTH_*`, settings.rs
/// `TRANSCRIPT_WIDTH_*`).
const WIDTH_SLIDER_WIDTH: f32 = 196.0;
const WIDTH_TRACK: f32 = 4.0;
const WIDTH_KNOB: f32 = 14.0;
const WIDTH_VALUE_W: f32 = 56.0;
pub const CONVERSATION_WIDTH_MIN: f32 = 560.0;
pub const CONVERSATION_WIDTH_MAX: f32 = 1200.0;
pub const CONVERSATION_WIDTH_DEFAULT: f32 = 736.0;
const CONVERSATION_WIDTH_STEP: f32 = 16.0;

/// The interface size ladder (Pink typography.rs `UiFontSize::ALL`).
pub const INTERFACE_SIZES: [u8; 7] = [12, 13, 14, 15, 16, 18, 20];

/// A press on the trigger that lands this soon after the menu was
/// dismissed by that same press leaves it closed (Pink `toggle_font_menu`).
const REOPEN_GUARD: Duration = Duration::from_millis(400);

// ---------------------------------------------------------------------------
// Pink's color model (theme crate `Color`): u8 channels, WCAG contrast
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    const fn hex(hex: u32) -> Self {
        Self {
            r: (hex >> 16) as u8,
            g: (hex >> 8) as u8,
            b: hex as u8,
        }
    }

    fn mix(self, other: Self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
        Self {
            r: mix(self.r, other.r),
            g: mix(self.g, other.g),
            b: mix(self.b, other.b),
        }
    }

    fn luminance(self) -> f32 {
        let linear = |channel: u8| {
            let value = channel as f32 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    fn contrast(self, background: Self) -> f32 {
        let (a, b) = (self.luminance(), background.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Moves toward black or white until `minimum` is met (Pink
    /// `Color::ensure_contrast`).
    fn ensure_contrast(self, background: Self, minimum: f32) -> Self {
        if self.contrast(background) >= minimum {
            return self;
        }
        let target = if Self::BLACK.contrast(background) >= Self::WHITE.contrast(background) {
            Self::BLACK
        } else {
            Self::WHITE
        };
        for step in 1..=20 {
            let candidate = self.mix(target, step as f32 / 20.0);
            if candidate.contrast(background) >= minimum {
                return candidate;
            }
        }
        target
    }

    fn hsla(self) -> Hsla {
        self.alpha(1.0)
    }

    /// With an alpha stored as Pink stores it, in a u8.
    fn alpha(self, alpha: f32) -> Hsla {
        Rgba {
            r: self.r as f32 / 255.0,
            g: self.g as f32 / 255.0,
            b: self.b as f32 / 255.0,
            a: (alpha * 255.0).round() / 255.0,
        }
        .into()
    }
}

/// An accent as Pink paints it (theme crate `AccentRoles::derive`): the
/// primary held to 3:1 on the theme's background, its wash, and the three
/// glyph rows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Accent {
    primary: Hsla,
    wash: Hsla,
    glyph: [Hsla; 3],
}

impl Accent {
    fn derive(color: Rgb, dark: bool, background: Rgb) -> Self {
        let primary = color.ensure_contrast(background, 3.0);
        let light = primary.mix(
            if dark { Rgb::WHITE } else { background },
            if dark { 0.28 } else { 0.18 },
        );
        let deep = primary.mix(Rgb::BLACK, if dark { 0.18 } else { 0.26 });
        Self {
            primary: primary.hsla(),
            wash: primary.alpha(if dark { 0.22 } else { 0.12 }),
            glyph: [light.hsla(), primary.hsla(), deep.hsla()],
        }
    }
}

// ---------------------------------------------------------------------------
// Public data
// ---------------------------------------------------------------------------

/// Follow the system, or keep the app light or dark (Pink `AppearanceMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppearanceMode {
    #[default]
    System,
    Light,
    Dark,
}

impl AppearanceMode {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::System => ICON_SYSTEM,
            Self::Light => ICON_LIGHT,
            Self::Dark => ICON_DARK,
        }
    }
}

/// The theme's own accent or one of Pink's presets (Pink `AccentSelection`
/// over `AccentPreset::ALL`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccentChoice {
    #[default]
    ThemeDefault,
    Violet,
    Orange,
    Amber,
    Green,
    Cyan,
    Blue,
    Pink,
}

impl AccentChoice {
    pub const ALL: [Self; 8] = [
        Self::ThemeDefault,
        Self::Violet,
        Self::Orange,
        Self::Amber,
        Self::Green,
        Self::Cyan,
        Self::Blue,
        Self::Pink,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ThemeDefault => "Theme default",
            Self::Violet => "Violet",
            Self::Orange => "Orange",
            Self::Amber => "Amber",
            Self::Green => "Green",
            Self::Cyan => "Cyan",
            Self::Blue => "Blue",
            Self::Pink => "Pink",
        }
    }

    /// The preset's authored color per appearance (Pink
    /// `AccentPreset::color`), `None` for the theme's own.
    fn color(self, dark: bool) -> Option<Rgb> {
        let (on_dark, on_light) = match self {
            Self::ThemeDefault => return None,
            Self::Violet => (0x8b7cf6, 0x5b43e8),
            Self::Orange => (0xfb923c, 0xc2410c),
            Self::Amber => (0xfbbf24, 0xa16207),
            Self::Green => (0x4ade80, 0x15803d),
            Self::Cyan => (0x22d3ee, 0x0e7490),
            Self::Blue => (0x60a5fa, 0x2563eb),
            Self::Pink => (0xf472b6, 0xbe185d),
        };
        Some(Rgb::hex(if dark { on_dark } else { on_light }))
    }

    /// Pink `accent_helper`.
    fn helper(self) -> String {
        match self {
            Self::ThemeDefault => "Theme default · Uses the palette's intended color.".into(),
            preset => format!(
                "{} · Controls, glyphs, selections, code, and activity.",
                preset.label()
            ),
        }
    }
}

/// Frosted or opaque surfaces (Pink `SurfacePreference`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfacePreference {
    #[default]
    ThemeDefault,
    Frosted,
    Opaque,
}

impl SurfacePreference {
    pub const ALL: [Self; 3] = [Self::ThemeDefault, Self::Frosted, Self::Opaque];

    pub fn label(self) -> &'static str {
        match self {
            Self::ThemeDefault => "Theme default",
            Self::Frosted => "Frosted",
            Self::Opaque => "Opaque",
        }
    }

    /// Pink `surface_helper`.
    fn helper(self, theme_frosted: bool) -> String {
        match self {
            Self::ThemeDefault => format!(
                "Uses this theme's {} default.",
                if theme_frosted { "frosted" } else { "opaque" }
            ),
            Self::Frosted => "Theme-colored glass where supported.".into(),
            Self::Opaque => "Solid surfaces for every theme.".into(),
        }
    }
}

/// One theme variant, as its seeds (Pink theme crate `builtins.rs`
/// `Seeds`): the colors the previews and palette chips paint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeVariant {
    pub id: SharedString,
    pub name: SharedString,
    pub dark: bool,
    pub background: u32,
    pub shell: u32,
    pub text: u32,
    pub accent: u32,
    pub frosted: bool,
}

impl ThemeVariant {
    /// Colors as `0xRRGGBB`.
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        dark: bool,
        background: u32,
        shell: u32,
        text: u32,
        accent: u32,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            dark,
            background,
            shell,
            text,
            accent,
            frosted: false,
        }
    }

    /// The theme recommends the frosted treatment.
    pub fn frosted(mut self) -> Self {
        self.frosted = true;
        self
    }

    /// Pink's built-in registry, in its menu order.
    pub fn builtins() -> Vec<Self> {
        BUILTINS
            .iter()
            .map(|(id, name, dark, frosted, bg, shell, text, accent)| Self {
                id: (*id).into(),
                name: (*name).into(),
                dark: *dark,
                background: *bg,
                shell: *shell,
                text: *text,
                accent: *accent,
                frosted: *frosted,
            })
            .collect()
    }

    /// The preview tones (Pink `Theme::for_selection` over this variant):
    /// shell, background, the hairline edge and text.
    fn tones(&self) -> Tones {
        let edge = if self.dark { Rgb::WHITE } else { Rgb::BLACK };
        Tones {
            surface: Rgb::hex(self.shell).hsla(),
            bg: Rgb::hex(self.background).hsla(),
            border: edge.alpha(if self.dark { 0.10 } else { 0.12 }),
            text: Rgb::hex(self.text).hsla(),
        }
    }

    /// This variant's accent for `choice` (Pink `ThemeVariant::accent_for`).
    fn accent(&self, choice: AccentChoice) -> Accent {
        let color = choice.color(self.dark).unwrap_or(Rgb::hex(self.accent));
        Accent::derive(color, self.dark, Rgb::hex(self.background))
    }
}

/// id, name, dark, frosted, background, shell, text, accent (Pink
/// builtins.rs, registry order).
#[allow(clippy::type_complexity)]
const BUILTINS: [(&str, &str, bool, bool, u32, u32, u32, u32); 30] = [
    (
        "pink-light",
        "Pink Light",
        false,
        true,
        0xfbfcfe,
        0xf0f3f7,
        0x202631,
        0x5b43e8,
    ),
    (
        "pink-dark",
        "Pink Dark",
        true,
        true,
        0x060606,
        0x0d0d0d,
        0xe8e8ea,
        0x8b7cf6,
    ),
    (
        "vscode-light-plus",
        "Light+",
        false,
        false,
        0xffffff,
        0xf3f3f3,
        0x1f1f1f,
        0x0078d4,
    ),
    (
        "vscode-dark-plus",
        "Dark+",
        true,
        false,
        0x1e1e1e,
        0x181818,
        0xd4d4d4,
        0x007acc,
    ),
    (
        "catppuccin-latte",
        "Catppuccin Latte",
        false,
        false,
        0xeff1f5,
        0xe6e9ef,
        0x4c4f69,
        0x8839ef,
    ),
    (
        "catppuccin-mocha",
        "Catppuccin Mocha",
        true,
        false,
        0x1e1e2e,
        0x181825,
        0xcdd6f4,
        0xcba6f7,
    ),
    (
        "tokyo-night-light",
        "Tokyo Night Light",
        false,
        false,
        0xd5d6db,
        0xcbccd1,
        0x343b59,
        0x34548a,
    ),
    (
        "tokyo-night",
        "Tokyo Night",
        true,
        false,
        0x1a1b26,
        0x16161e,
        0xc0caf5,
        0x7aa2f7,
    ),
    (
        "dracula", "Dracula", true, false, 0x282a36, 0x21222c, 0xf8f8f2, 0xbd93f9,
    ),
    (
        "github-light",
        "GitHub Light",
        false,
        false,
        0xffffff,
        0xf6f8fa,
        0x1f2328,
        0x0969da,
    ),
    (
        "github-dark",
        "GitHub Dark",
        true,
        false,
        0x0d1117,
        0x010409,
        0xe6edf3,
        0x2f81f7,
    ),
    (
        "ayu-light",
        "Ayu Light",
        false,
        false,
        0xfcfcfc,
        0xf8f9fa,
        0x5c6166,
        0xf29718,
    ),
    (
        "ayu-dark", "Ayu Dark", true, false, 0x10141c, 0x0d1017, 0xbfbdb6, 0xe6b450,
    ),
    (
        "ayu-mirage",
        "Ayu Mirage",
        true,
        false,
        0x242936,
        0x1f2430,
        0xcccac2,
        0xffcc66,
    ),
    (
        "gruvbox-light",
        "Gruvbox Light",
        false,
        false,
        0xfbf1c7,
        0xf2e5bc,
        0x3c3836,
        0x076678,
    ),
    (
        "gruvbox-dark",
        "Gruvbox Dark",
        true,
        false,
        0x282828,
        0x1d2021,
        0xebdbb2,
        0x8ec07c,
    ),
    (
        "rose-pine-dawn",
        "Rosé Pine Dawn",
        false,
        false,
        0xfaf4ed,
        0xf2e9e1,
        0x575279,
        0x907aa9,
    ),
    (
        "rose-pine-moon",
        "Rosé Pine Moon",
        true,
        false,
        0x232136,
        0x1f1d2e,
        0xe0def4,
        0xc4a7e7,
    ),
    (
        "nord", "Nord", true, false, 0x2e3440, 0x292e39, 0xd8dee9, 0x88c0d0,
    ),
    (
        "one-dark-pro",
        "One Dark Pro",
        true,
        false,
        0x282c34,
        0x21252b,
        0xabb2bf,
        0x528bff,
    ),
    (
        "atom-one-dark",
        "Atom One Dark",
        true,
        false,
        0x282c34,
        0x21252b,
        0xabb2bf,
        0x528bff,
    ),
    (
        "night-owl-light",
        "Night Owl Light",
        false,
        false,
        0xfbfbfb,
        0xf0f0f0,
        0x403f53,
        0x4876d6,
    ),
    (
        "night-owl",
        "Night Owl",
        true,
        false,
        0x011627,
        0x01111d,
        0xd6deeb,
        0x82aaff,
    ),
    (
        "winter-light",
        "Winter is Coming Light",
        false,
        false,
        0xffffff,
        0xf3f3f3,
        0x1857a4,
        0x2f86d2,
    ),
    (
        "winter-dark-blue",
        "Winter is Coming Dark Blue",
        true,
        false,
        0x011627,
        0x01111d,
        0xd6deeb,
        0x219fd5,
    ),
    (
        "palenight",
        "Palenight",
        true,
        false,
        0x292d3e,
        0x232635,
        0xeeffff,
        0x82aaff,
    ),
    (
        "synthwave-84",
        "SynthWave '84",
        true,
        false,
        0x262335,
        0x241b2f,
        0xffffff,
        0xff7edb,
    ),
    (
        "shades-of-purple",
        "Shades of Purple",
        true,
        false,
        0x2d2b55,
        0x222244,
        0xffffff,
        0xfad000,
    ),
    (
        "cobalt2", "Cobalt2", true, false, 0x193549, 0x15232d, 0xffffff, 0xffc600,
    ),
    (
        "andromeda",
        "Andromeda",
        true,
        false,
        0x23262e,
        0x1e2027,
        0xd5ced9,
        0x00e8c6,
    ),
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Tones {
    surface: Hsla,
    bg: Hsla,
    border: Hsla,
    text: Hsla,
}

/// A font family in the interface font menu (Pink `UiFontFamily`); one
/// that isn't on this device is listed but can't be picked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFamily {
    pub label: SharedString,
    pub available: bool,
}

impl FontFamily {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            available: true,
        }
    }

    pub fn unavailable(mut self) -> Self {
        self.available = false;
        self
    }
}

/// One edit, as `on_change` reports it.
#[derive(Clone, Debug, PartialEq)]
pub enum AppearanceChange {
    Mode(AppearanceMode),
    LightTheme(SharedString),
    DarkTheme(SharedString),
    Accent(AccentChoice),
    Surface(SurfacePreference),
    Font(SharedString),
    /// The interface size in px, one of [`INTERFACE_SIZES`].
    InterfaceSize(u8),
    ConversationWidth(f32),
}

type ChangeHandler = Rc<dyn Fn(&AppearanceChange, &mut Window, &mut App)>;

/// Settings › Appearance. See the module docs.
#[derive(IntoElement)]
pub struct AppearancePicker {
    id: ElementId,
    mode: AppearanceMode,
    themes: Vec<ThemeVariant>,
    light_theme: SharedString,
    dark_theme: SharedString,
    accent: AccentChoice,
    surface: SurfacePreference,
    fonts: Vec<FontFamily>,
    font: SharedString,
    interface_size: u8,
    width: f32,
    on_change: Option<ChangeHandler>,
}

impl AppearancePicker {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            mode: AppearanceMode::System,
            themes: ThemeVariant::builtins(),
            light_theme: "pink-light".into(),
            dark_theme: "pink-dark".into(),
            accent: AccentChoice::ThemeDefault,
            surface: SurfacePreference::ThemeDefault,
            fonts: vec![
                FontFamily::new("Geist"),
                FontFamily::new("Geist Mono"),
                FontFamily::new("System UI"),
            ],
            font: "Geist".into(),
            interface_size: 16,
            width: CONVERSATION_WIDTH_DEFAULT,
            on_change: None,
        }
    }

    pub fn mode(mut self, mode: AppearanceMode) -> Self {
        self.mode = mode;
        self
    }

    /// The theme catalog; [`ThemeVariant::builtins`] by default.
    pub fn themes(mut self, themes: impl IntoIterator<Item = ThemeVariant>) -> Self {
        self.themes = themes.into_iter().collect();
        self
    }

    pub fn light_theme(mut self, id: impl Into<SharedString>) -> Self {
        self.light_theme = id.into();
        self
    }

    pub fn dark_theme(mut self, id: impl Into<SharedString>) -> Self {
        self.dark_theme = id.into();
        self
    }

    pub fn accent(mut self, accent: AccentChoice) -> Self {
        self.accent = accent;
        self
    }

    pub fn surface(mut self, surface: SurfacePreference) -> Self {
        self.surface = surface;
        self
    }

    /// The families the interface font menu lists, in its order.
    pub fn fonts(mut self, fonts: impl IntoIterator<Item = FontFamily>) -> Self {
        self.fonts = fonts.into_iter().collect();
        self
    }

    pub fn font(mut self, label: impl Into<SharedString>) -> Self {
        self.font = label.into();
        self
    }

    /// In px; snaps to the nearest of [`INTERFACE_SIZES`].
    pub fn interface_size(mut self, px: u8) -> Self {
        self.interface_size = px;
        self
    }

    /// In px; held to 560–1200 on a 16 px step.
    pub fn conversation_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&AppearanceChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Clamp to the range and snap to the step from its minimum (Pink
/// `normalize_transcript_width`).
fn normalize_width(width: f32) -> f32 {
    let width = if width.is_finite() {
        width.clamp(CONVERSATION_WIDTH_MIN, CONVERSATION_WIDTH_MAX)
    } else {
        CONVERSATION_WIDTH_DEFAULT
    };
    CONVERSATION_WIDTH_MIN
        + ((width - CONVERSATION_WIDTH_MIN) / CONVERSATION_WIDTH_STEP).round()
            * CONVERSATION_WIDTH_STEP
}

/// The ladder rung nearest `px` (Pink `UiFontSize::normalized`).
fn size_ix(px: u8) -> usize {
    INTERFACE_SIZES
        .iter()
        .enumerate()
        .min_by_key(|(_, size)| size.abs_diff(px))
        .map_or(4, |(ix, _)| ix)
}

/// Families matching `query`: prefix matches first, then substring
/// matches, catalog order within each (Pink `filter_families` /
/// `popover::filter_indices`).
fn filter_fonts(query: &str, fonts: &[FontFamily]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut ranked: Vec<(usize, usize)> = fonts
        .iter()
        .enumerate()
        .filter_map(|(ix, font)| {
            if query.is_empty() {
                return Some((1, ix));
            }
            let label = font.label.to_lowercase();
            if label.starts_with(&query) {
                Some((0, ix))
            } else if label.contains(&query) {
                Some((1, ix))
            } else {
                None
            }
        })
        .collect();
    ranked.sort_by_key(|&(rank, ix)| (rank, ix));
    ranked.into_iter().map(|(_, ix)| ix).collect()
}

/// The next family a step lands on, skipping ones that aren't installed and
/// stopping at the ends (Pink `step_font`).
fn step_font(
    current: Option<usize>,
    delta: isize,
    visible: &[usize],
    fonts: &[FontFamily],
) -> Option<usize> {
    if visible.is_empty() {
        return current;
    }
    let at = current
        .and_then(|font| visible.iter().position(|ix| *ix == font))
        .unwrap_or_default() as isize;
    let mut pos = at + delta.signum();
    while (0..visible.len() as isize).contains(&pos) {
        let candidate = visible[pos as usize];
        if fonts[candidate].available {
            return Some(candidate);
        }
        pos += delta.signum();
    }
    Some(visible[at as usize])
}

fn first_available(visible: &[usize], fonts: &[FontFamily]) -> Option<usize> {
    visible.iter().copied().find(|ix| fonts[*ix].available)
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    LightTheme,
    DarkTheme,
    Font,
    Size,
}

struct PickerState {
    seeded: bool,
    mode: AppearanceMode,
    light_theme: SharedString,
    dark_theme: SharedString,
    accent: AccentChoice,
    surface: SurfacePreference,
    font: SharedString,
    size: usize,
    width: f32,
    menu: Option<Menu>,
    closing: Option<(Menu, Instant)>,
    reap: Option<Task<()>>,
    pressed_while_open: bool,
    dismissed_at: Option<(Menu, Instant)>,
    /// The highlighted family (an index into the fonts) and size rung.
    font_cursor: Option<usize>,
    size_cursor: usize,
    query: SharedString,
    search: Entity<TextInput>,
    _subscription: Subscription,
    font_focus: FocusHandle,
    size_focus: FocusHandle,
    slider_focus: FocusHandle,
    dragging: bool,
    slider_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    fonts: Rc<Vec<FontFamily>>,
    handler: Option<ChangeHandler>,
}

impl PickerState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new(window, cx).placeholder("Search fonts"));
        let subscription =
            cx.subscribe_in(&search, window, |s: &mut Self, _, event, window, cx| {
                s.on_search(event, window, cx)
            });
        Self {
            seeded: false,
            mode: AppearanceMode::System,
            light_theme: SharedString::default(),
            dark_theme: SharedString::default(),
            accent: AccentChoice::ThemeDefault,
            surface: SurfacePreference::ThemeDefault,
            font: SharedString::default(),
            size: 4,
            width: CONVERSATION_WIDTH_DEFAULT,
            menu: None,
            closing: None,
            reap: None,
            pressed_while_open: false,
            dismissed_at: None,
            font_cursor: None,
            size_cursor: 4,
            query: SharedString::default(),
            search,
            _subscription: subscription,
            font_focus: cx.focus_handle(),
            size_focus: cx.focus_handle(),
            slider_focus: cx.focus_handle(),
            dragging: false,
            slider_bounds: Rc::default(),
            fonts: Rc::new(Vec::new()),
            handler: None,
        }
    }

    fn seed(&mut self, picker: &AppearancePicker) {
        self.mode = picker.mode;
        self.light_theme = picker.light_theme.clone();
        self.dark_theme = picker.dark_theme.clone();
        self.accent = picker.accent;
        self.surface = picker.surface;
        self.font = picker.font.clone();
        self.size = size_ix(picker.interface_size);
        self.size_cursor = self.size;
        self.width = normalize_width(picker.width);
        self.seeded = true;
    }

    fn font_ix(&self) -> Option<usize> {
        self.fonts.iter().position(|f| f.label == self.font)
    }

    fn visible_fonts(&self) -> Vec<usize> {
        filter_fonts(&self.query, &self.fonts)
    }

    fn open(&mut self, menu: Menu) {
        if let Some(open) = self.menu
            && open != menu
        {
            self.menu = None;
            self.closing = None;
        }
        self.menu = Some(menu);
        self.closing = None;
        self.reap = None;
    }

    /// Starts the exit fade; the card unmounts once it has run.
    fn close(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        let since = Instant::now();
        self.closing = Some((menu, since));
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        self.reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                if s.closing.is_some_and(|(_, at)| at == since) {
                    s.closing = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// A press outside the card closed it; the same press on its trigger
    /// must not reopen it.
    fn dismiss(&mut self, menu: Menu, cx: &mut Context<Self>) {
        if self.menu == Some(menu) {
            self.dismissed_at = Some((menu, Instant::now()));
            self.close(cx);
        }
    }

    fn just_dismissed(&mut self, menu: Menu) -> bool {
        self.dismissed_at
            .take()
            .is_some_and(|(m, at)| m == menu && at.elapsed() < REOPEN_GUARD)
    }

    /// Pink `toggle_font_menu`: opening clears the filter, highlights the
    /// family in use and moves the keyboard into the filter.
    fn toggle_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let just = self.just_dismissed(Menu::Font);
        if self.menu == Some(Menu::Font) {
            self.close(cx);
        } else if !just {
            self.query = SharedString::default();
            self.search.update(cx, |input, cx| input.clear(cx));
            self.font_cursor = self.font_ix();
            self.open(Menu::Font);
            let search = self.search.clone();
            TextInput::focus(&search, window, cx);
        }
        cx.notify();
    }

    /// Pink `toggle_size_menu`.
    fn toggle_size(&mut self, cx: &mut Context<Self>) {
        let just = self.just_dismissed(Menu::Size);
        if self.menu == Some(Menu::Size) {
            self.close(cx);
        } else if !just {
            self.size_cursor = self.size;
            self.open(Menu::Size);
        }
        cx.notify();
    }

    fn commit_font(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AppearanceChange> {
        let font = self.fonts.get(ix).filter(|f| f.available)?.label.clone();
        self.close(cx);
        window.focus(&self.font_focus, cx);
        if font == self.font {
            return None;
        }
        self.font = font.clone();
        Some(AppearanceChange::Font(font))
    }

    fn commit_size(&mut self, cx: &mut Context<Self>) -> Option<AppearanceChange> {
        let ix = self.size_cursor.min(INTERFACE_SIZES.len() - 1);
        self.close(cx);
        if ix == self.size {
            return None;
        }
        self.size = ix;
        Some(AppearanceChange::InterfaceSize(INTERFACE_SIZES[ix]))
    }

    fn set_width(&mut self, width: f32) -> Option<AppearanceChange> {
        let width = normalize_width(width);
        if (width - self.width).abs() < 0.5 {
            return None;
        }
        self.width = width;
        Some(AppearanceChange::ConversationWidth(width))
    }

    /// The width under the pointer: the knob's half-width insets the track
    /// at either end (Pink `drag_width`).
    fn width_at(&self, x: Pixels) -> Option<f32> {
        let bounds = self.slider_bounds.get()?;
        let inset = WIDTH_KNOB / 2.0;
        let fraction = (f32::from(x - bounds.left()) - inset)
            / (f32::from(bounds.size.width) - 2.0 * inset).max(1.0);
        Some(
            CONVERSATION_WIDTH_MIN
                + fraction.clamp(0.0, 1.0) * (CONVERSATION_WIDTH_MAX - CONVERSATION_WIDTH_MIN),
        )
    }

    /// Keys from the open font menu's filter field (Pink `on_font_key_down`
    /// while open).
    fn on_search(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu != Some(Menu::Font) {
            return;
        }
        match event {
            InputEvent::Change(text) => {
                // Pink `clamp_highlight`: the highlight stays inside the
                // filtered list.
                self.query = text.clone();
                let visible = self.visible_fonts();
                if !self.font_cursor.is_some_and(|c| visible.contains(&c)) {
                    self.font_cursor = first_available(&visible, &self.fonts);
                }
                cx.notify();
            }
            InputEvent::MoveUp | InputEvent::MoveDown => {
                let delta = if matches!(event, InputEvent::MoveUp) {
                    -1
                } else {
                    1
                };
                let visible = self.visible_fonts();
                self.font_cursor = step_font(self.font_cursor, delta, &visible, &self.fonts);
                cx.notify();
            }
            InputEvent::Submit(_) => {
                if let Some(ix) = self.font_cursor
                    && let Some(change) = self.commit_font(ix, window, cx)
                    && let Some(handler) = self.handler.clone()
                {
                    handler(&change, window, cx);
                }
            }
            InputEvent::Escape => {
                self.font_cursor = self.font_ix();
                self.close(cx);
                window.focus(&self.font_focus, cx);
            }
            _ => {}
        }
    }
}

/// Runs `edit` on the state, then reports its change.
fn apply(
    state: &Entity<PickerState>,
    window: &mut Window,
    cx: &mut App,
    edit: impl FnOnce(
        &mut PickerState,
        &mut Window,
        &mut Context<PickerState>,
    ) -> Option<AppearanceChange>,
) {
    let (change, handler) = state.update(cx, |s, cx| {
        let change = edit(s, window, cx);
        cx.notify();
        (change, s.handler.clone())
    });
    if let (Some(change), Some(handler)) = (change, handler) {
        handler(&change, window, cx);
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

/// What one frame paints from.
struct Frame<'a> {
    key: &'a str,
    t: &'a Pink,
    state: &'a Entity<PickerState>,
    themes: &'a [ThemeVariant],
    light: ThemeVariant,
    dark: ThemeVariant,
    /// The page's accent: the chosen one on the current appearance's theme.
    accent: Accent,
    menu: Option<Menu>,
    closing: Option<(Menu, Instant)>,
    reduced: bool,
}

impl Frame<'_> {
    /// The menu's exit progress while it fades, `None` while it is open.
    fn exit(&self, menu: Menu) -> Option<Option<f32>> {
        if self.menu == Some(menu) {
            return Some(None);
        }
        self.closing.filter(|(m, _)| *m == menu).map(|(_, since)| {
            Some(if self.reduced {
                1.0
            } else {
                motion::menu_exit_progress(since)
            })
        })
    }
}

fn variant_or(themes: &[ThemeVariant], id: &str, dark: bool) -> ThemeVariant {
    themes
        .iter()
        .find(|v| v.id.as_ref() == id && v.dark == dark)
        .or_else(|| themes.iter().find(|v| v.dark == dark))
        .cloned()
        .unwrap_or_else(|| {
            let (bg, shell, text, accent) = if dark {
                (0x060606, 0x0d0d0d, 0xe8e8ea, 0x8b7cf6)
            } else {
                (0xfbfcfe, 0xf0f3f7, 0x202631, 0x5b43e8)
            };
            ThemeVariant::new(
                if dark { "pink-dark" } else { "pink-light" },
                if dark { "Pink Dark" } else { "Pink Light" },
                dark,
                bg,
                shell,
                text,
                accent,
            )
            .frosted()
        })
}

impl RenderOnce for AppearancePicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            PickerState::new,
        );
        let fonts = Rc::new(self.fonts.clone());
        let handler = self.on_change.clone();
        let search = state.update(cx, |s, _| {
            if !s.seeded {
                s.seed(&self);
            }
            s.fonts = fonts.clone();
            s.handler = handler;
            s.search.clone()
        });
        search.update(cx, |input, _| {
            input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
        });

        let s = state.read(cx);
        let light = variant_or(&self.themes, &s.light_theme, false);
        let dark = variant_or(&self.themes, &s.dark_theme, true);
        let current = if t.dark { &dark } else { &light };
        let accent = current.accent(s.accent);
        let mode = s.mode;
        let accent_choice = s.accent;
        let surface = s.surface;
        let theme_frosted = current.frosted;
        let font = s.font.clone();
        let size = s.size;
        let size_cursor = s.size_cursor;
        let font_cursor = s.font_cursor;
        let query = s.query.clone();
        let width = s.width;
        let dragging = s.dragging;
        let font_focus = s.font_focus.clone();
        let size_focus = s.size_focus.clone();
        let slider_focus = s.slider_focus.clone();
        let slider_bounds = s.slider_bounds.clone();
        let visible = s.visible_fonts();
        let frame = Frame {
            key: &key,
            t: &t,
            state: &state,
            themes: &self.themes,
            light: light.clone(),
            dark: dark.clone(),
            accent,
            menu: s.menu,
            closing: s.closing,
            reduced: motion::reduced_motion(cx),
        };

        // ── Theme (Pink appearance.rs `theme_section`) ──────────────────
        let cards: Vec<AnyElement> = AppearanceMode::ALL
            .into_iter()
            .map(|m| option_card(&frame, m, m == mode).into_any_element())
            .collect();
        let swatches: Vec<AnyElement> = AccentChoice::ALL
            .into_iter()
            .map(|choice| accent_swatch(&frame, current, choice, choice == accent_choice))
            .collect();
        let glass = {
            let state = state.clone();
            pink::segmented(
                format!("{key}-surface"),
                SurfacePreference::ALL
                    .into_iter()
                    .map(|s| Segment::new(s.label().to_lowercase().replace(' ', "-"), s.label()))
                    .collect(),
                SurfacePreference::ALL
                    .iter()
                    .position(|s| *s == surface)
                    .unwrap_or(0),
                SEGMENTED_GLASS_WIDTH,
                ControlSize::Md,
                &t,
                move |ix, window, cx| {
                    let choice = SurfacePreference::ALL[*ix];
                    apply(&state, window, cx, |s, _, _| {
                        (s.surface != choice).then(|| {
                            s.surface = choice;
                            AppearanceChange::Surface(choice)
                        })
                    })
                },
            )
            .aria_label("Glass")
        };
        let theme_card = pink::section_card(&t)
            .mt(px(0.0))
            .child(pink::section_header("Theme", None, &t))
            .child(
                pink::card_row(false, &t)
                    .child(pink::row_text(
                        "Appearance",
                        Some("Follow macOS, or keep PINK light or dark.".into()),
                        &t,
                    ))
                    .child(
                        pink::row_block().child(
                            div()
                                .flex()
                                .flex_row()
                                .items_start()
                                .gap(px(OPTION_CARD_GAP))
                                .w_full()
                                .children(cards),
                        ),
                    ),
            )
            .child(setting_row(
                "Light theme",
                "Used whenever PINK is light.",
                vec![theme_selector(&frame, false, window)],
                &t,
            ))
            .child(setting_row(
                "Dark theme",
                "Used whenever PINK is dark.",
                vec![theme_selector(&frame, true, window)],
                &t,
            ))
            .child(
                pink::card_row(false, &t)
                    .child(pink::row_text(
                        "Accent color",
                        Some(accent_choice.helper().into()),
                        &t,
                    ))
                    .child(pink::row_block().gap(px(6.0)).children(swatches)),
            )
            .child(setting_row(
                "Glass",
                surface.helper(theme_frosted),
                vec![glass.into_any_element()],
                &t,
            ));

        // ── Text (Pink appearance.rs `text_section`) ────────────────────
        let font_picker = font_picker(
            &frame,
            &fonts,
            &font,
            font_cursor,
            &visible,
            &query,
            &search,
            &font_focus,
        );
        let size_picker = size_picker(&frame, size, size_cursor, &size_focus);
        let width_control = width_control(&frame, width, dragging, &slider_focus, slider_bounds);
        let text_card = pink::section_card(&t)
            .child(pink::section_header("Text", None, &t))
            .child(setting_row(
                "Interface font",
                "Menus, sidebars, and conversation text. The size scales the whole interface.",
                vec![font_picker, size_picker],
                &t,
            ))
            .child(setting_row(
                "Conversation width",
                "Maximum width of messages. Adapts to smaller windows.",
                vec![width_control],
                &t,
            ));

        div()
            .id(self.id)
            .w(px(CARD_W))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(theme_card)
            .child(text_card)
    }
}

/// One settings row (Pink appearance.rs `setting_row`): the wrapping text
/// at the left, the controls in the trailing column.
fn setting_row(
    title: &'static str,
    description: impl Into<SharedString>,
    controls: Vec<AnyElement>,
    t: &Pink,
) -> Div {
    pink::card_row(false, t)
        .child(pink::row_text(title, Some(description.into()), t))
        .child(pink::row_controls().children(controls))
}

/// The selection ring (Pink `button::selection_ring`) in a given accent: a
/// 2 px ring 2 px outside the child, its corner `inner + 4`, the space kept
/// when unselected so choosing never shifts the layout.
fn ring(selected: bool, inner_radius: f32, accent: Hsla, child: impl IntoElement) -> Div {
    div()
        .flex_none()
        .p(px(2.0))
        .border_2()
        .border_color(if selected {
            accent
        } else {
            gpui::transparent_black()
        })
        .rounded(px(inner_radius + 4.0))
        .child(child)
}

// ── Option cards ────────────────────────────────────────────────────────

/// One filled bar of a miniature (Pink appearance.rs `bar`): 5 tall, radius 3.
fn bar(fraction: f32, tone: Hsla) -> Div {
    div()
        .h(px(5.0))
        .w(gpui::relative(fraction))
        .rounded(px(3.0))
        .bg(tone)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Corners {
    All,
    Left,
    Right,
}

/// A window in miniature (Pink appearance.rs `miniature`): a 44 sidebar of
/// bars on the shell tone beside a bordered content sheet of bars on the
/// background, the bars in `text` at 22% and 34%.
fn miniature(tones: Tones, corners: Corners) -> Div {
    let line = tones.text.opacity(0.22);
    let strong = tones.text.opacity(0.34);
    let r = px(OPTION_CARD_RADIUS);
    let root = div().size_full().flex().flex_row().bg(tones.surface);
    let root = match corners {
        Corners::All => root.rounded(r),
        Corners::Left => root.rounded_tl(r).rounded_bl(r),
        Corners::Right => root.rounded_tr(r).rounded_br(r),
    };
    root.child(
        div()
            .w(px(44.0))
            .h_full()
            .flex_none()
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(7.0))
            .px(px(8.0))
            .pt(px(14.0))
            .child(bar(0.70, strong))
            .child(bar(1.0, line))
            .child(bar(0.85, line))
            .child(bar(1.0, line)),
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .my(px(8.0))
            .mr(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(tones.border)
            .bg(tones.bg)
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(7.0))
            .p(px(10.0))
            .child(bar(0.62, strong))
            .child(bar(0.88, line))
            .child(bar(0.76, line))
            .child(bar(0.52, line)),
    )
}

/// The preview for a mode (Pink appearance.rs `preview`): System splits
/// the light and dark miniatures down the middle.
fn preview(mode: AppearanceMode, light: &ThemeVariant, dark: &ThemeVariant) -> AnyElement {
    match mode {
        AppearanceMode::System => div()
            .size_full()
            .flex()
            .flex_row()
            .child(
                div()
                    .w_1_2()
                    .h_full()
                    .overflow_hidden()
                    .child(miniature(light.tones(), Corners::Left)),
            )
            .child(
                div()
                    .w_1_2()
                    .h_full()
                    .overflow_hidden()
                    .child(miniature(dark.tones(), Corners::Right)),
            )
            .into_any_element(),
        AppearanceMode::Light => miniature(light.tones(), Corners::All).into_any_element(),
        AppearanceMode::Dark => miniature(dark.tones(), Corners::All).into_any_element(),
    }
}

/// One option card (Pink widgets.rs `option_card`): a 148 tall preview
/// frame (radius 6, `border`) inside the selection ring, and the caption
/// under it, gap 8: a 16 glyph and the 13 label, medium `text` when chosen,
/// `text_muted` otherwise. Accent is reserved for the ring.
fn option_card(f: &Frame, mode: AppearanceMode, selected: bool) -> Stateful<Div> {
    let t = f.t;
    let ring_color = t.focus_ring();
    let state = f.state.clone();
    div()
        .id(SharedString::from(format!(
            "{}-mode-{}",
            f.key,
            mode.label()
        )))
        .role(Role::Button)
        .aria_label(format!("Appearance: {}", mode.label()))
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
        .tab_index(0)
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .rounded(px(OPTION_CARD_RADIUS + 4.0))
        .cursor_pointer()
        .focus_visible(move |style| pink::focus_ring(style, ring_color))
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, window, cx| {
            apply(&state, window, cx, |s, _, _| {
                (s.mode != mode).then(|| {
                    s.mode = mode;
                    AppearanceChange::Mode(mode)
                })
            })
        })
        .child(
            ring(
                selected,
                OPTION_CARD_RADIUS,
                f.accent.primary,
                div()
                    .h(px(OPTION_CARD_HEIGHT))
                    .w_full()
                    .rounded(px(OPTION_CARD_RADIUS))
                    .overflow_hidden()
                    .border_1()
                    .border_color(t.border)
                    .child(preview(mode, &f.light, &f.dark)),
            )
            .w_full(),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_size(px(13.0))
                .font_weight(if selected {
                    gpui::FontWeight::MEDIUM
                } else {
                    gpui::FontWeight::NORMAL
                })
                .text_color(if selected { t.text } else { t.text_muted })
                .child(
                    svg()
                        .path(mode.icon())
                        .size(px(pink::icon_size::MD))
                        .flex_none()
                        .text_color(if selected { t.text } else { t.text_muted }),
                )
                .child(mode.label()),
        )
}

// ── Accent swatches ─────────────────────────────────────────────────────

/// One accent swatch (Pink appearance.rs `accent_swatch`): a 30 square,
/// `p 2`, radius 8, `border` on `fill(Quaternary)`, holding the accent
/// (radius 6) or, for the theme's own, its wash with the three glyph rows
/// as 4 wide bars of 13, 16 and 11. The chosen one is ringed in its own
/// accent; its name is the tooltip.
fn accent_swatch(
    f: &Frame,
    current: &ThemeVariant,
    choice: AccentChoice,
    selected: bool,
) -> AnyElement {
    let t = f.t;
    let accent = current.accent(choice);
    let sample = match choice {
        AccentChoice::ThemeDefault => div()
            .size_full()
            .rounded(px(6.0))
            .bg(accent.wash)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(2.0))
            .child(
                div()
                    .w(px(4.0))
                    .h(px(13.0))
                    .rounded(px(2.0))
                    .bg(accent.glyph[0]),
            )
            .child(
                div()
                    .w(px(4.0))
                    .h(px(16.0))
                    .rounded(px(2.0))
                    .bg(accent.glyph[1]),
            )
            .child(
                div()
                    .w(px(4.0))
                    .h(px(11.0))
                    .rounded(px(2.0))
                    .bg(accent.glyph[2]),
            ),
        _ => div().size_full().rounded(px(6.0)).bg(accent.primary),
    };
    let ring_color = t.focus_ring();
    let state = f.state.clone();
    div()
        .id(SharedString::from(format!(
            "{}-accent-{}",
            f.key,
            choice.label()
        )))
        .role(Role::Button)
        .aria_label(format!("Accent color: {}", choice.label()))
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
        .tab_index(0)
        .flex_none()
        .rounded(px(12.0))
        .cursor_pointer()
        .focus_visible(move |style| pink::focus_ring(style, ring_color))
        .tooltip(pink::text_tooltip(choice.label()))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, window, cx| {
            apply(&state, window, cx, |s, _, _| {
                (s.accent != choice).then(|| {
                    s.accent = choice;
                    AppearanceChange::Accent(choice)
                })
            })
        })
        .child(ring(
            selected,
            8.0,
            accent.primary,
            div()
                .size(px(30.0))
                .p(px(2.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(t.border)
                .bg(t.fill(FillTier::Quaternary))
                .child(sample),
        ))
        .into_any_element()
}

// ── Menus ───────────────────────────────────────────────────────────────

/// A dropdown hung from the trigger, 6 below it and 8 inside the window
/// (Pink popover.rs `anchored_menu_below`): it drops 6 into place on
/// `MENU_IN`; while `exit` runs it fades on `MENU_OUT` under an occluder.
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

/// Swallows the press that dismisses the menu so nothing behind it acts on
/// it too (Pink `frosted_menu_rounded`'s guard).
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

/// A theme's palette chip (Pink appearance.rs `palette_preview`): 30×18,
/// radius 5, its `border`, thirds of shell, background and accent.
fn palette_preview(variant: &ThemeVariant) -> Div {
    let tones = variant.tones();
    let accent = variant.accent(AccentChoice::ThemeDefault).primary;
    div()
        .flex_none()
        .w(px(30.0))
        .h(px(18.0))
        .rounded(px(5.0))
        .overflow_hidden()
        .border_1()
        .border_color(tones.border)
        .flex()
        .child(div().w_1_3().h_full().bg(tones.surface))
        .child(div().w_1_3().h_full().bg(tones.bg))
        .child(div().w_1_3().h_full().bg(accent))
}

/// The light or dark theme pop-up (Pink appearance.rs
/// `render_theme_selector`): a 220 select with the theme's palette chip and
/// name; its 260 menu under a "Light themes" / "Dark themes" heading lists
/// every variant of that appearance with its chip, the current one checked.
fn theme_selector(f: &Frame, dark: bool, window: &Window) -> AnyElement {
    let t = f.t;
    let menu = if dark {
        Menu::DarkTheme
    } else {
        Menu::LightTheme
    };
    let selected = if dark { &f.dark } else { &f.light };
    let which = if dark { "dark" } else { "light" };
    let open = f.menu == Some(menu);
    let id = SharedString::from(format!("{}-{which}-theme", f.key));
    let press_state = f.state.clone();
    let click_state = f.state.clone();
    let mut trigger = pink::select_frame(id.clone(), ControlSize::Md, open, true, t)
        .aria_label(format!(
            "{} theme: {}",
            if dark { "Dark" } else { "Light" },
            selected.name
        ))
        .relative()
        .w(px(THEME_PICKER_WIDTH))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            press_state.update(cx, |s, _| s.pressed_while_open = s.menu == Some(menu))
        })
        .on_click(move |event: &ClickEvent, _, cx| {
            click_state.update(cx, |s, cx| {
                let was_open = std::mem::take(&mut s.pressed_while_open);
                // GPUI's keyboard click has no press, so it toggles.
                if event.is_keyboard() {
                    if s.menu == Some(menu) {
                        s.close(cx);
                    } else {
                        s.open(menu);
                    }
                } else if was_open {
                    s.close(cx);
                } else {
                    s.open(menu);
                }
                cx.notify();
            })
        })
        .on_key_down(pink::swallow_activation_key)
        .child(palette_preview(selected))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(selected.name.clone()),
        )
        .child(pink::select_chevron(t));
    if let Some(exit) = f.exit(menu) {
        let viewport = f32::from(window.viewport_size().height);
        let rows: Vec<AnyElement> = f
            .themes
            .iter()
            .filter(|v| v.dark == dark)
            .enumerate()
            .map(|(ix, variant)| {
                let active = variant.id == selected.id;
                let state = f.state.clone();
                let variant_id = variant.id.clone();
                let ring_color = t.focus_ring();
                pink::menu_row(format!("{id}-row-{ix}"), active, t)
                    .role(Role::MenuItemRadio)
                    .aria_label(format!(
                        "{} theme: {}",
                        if dark { "Dark" } else { "Light" },
                        variant.name
                    ))
                    .aria_toggled(if active {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .tab_index(0)
                    .focus_visible(move |style| pink::focus_ring(style, ring_color))
                    .on_key_down(pink::swallow_activation_key)
                    .on_click(move |_, window, cx| {
                        let id = variant_id.clone();
                        apply(&state, window, cx, |s, _, cx| {
                            s.close(cx);
                            let slot = if dark {
                                &mut s.dark_theme
                            } else {
                                &mut s.light_theme
                            };
                            (*slot != id).then(|| {
                                *slot = id.clone();
                                if dark {
                                    AppearanceChange::DarkTheme(id)
                                } else {
                                    AppearanceChange::LightTheme(id)
                                }
                            })
                        })
                    })
                    .child(palette_preview(variant))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(variant.name.clone()),
                    )
                    .when(active, |row| {
                        row.child(
                            pink::line_glyph(
                                pink::glyphs::CHECK,
                                14.0,
                                t.selected_row_icon(f.accent.primary),
                            )
                            .flex_none(),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let close_state = f.state.clone();
        let key_state = f.state.clone();
        let card = pink::popover_card(t)
            .id(SharedString::from(format!("{id}-card")))
            .role(Role::Menu)
            .w(px(THEME_MENU_WIDTH))
            .max_h(px((viewport - 96.0).max(160.0)))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .on_mouse_down_out(move |_, _, cx| close_state.update(cx, |s, cx| s.close(cx)))
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    key_state.update(cx, |s, cx| s.close(cx));
                    cx.stop_propagation();
                }
            })
            .child(pink::menu_heading(
                if dark { "Dark themes" } else { "Light themes" },
                t,
            ))
            .children(rows);
        trigger = trigger.child(anchored_menu_below(format!("{id}-menu").into(), exit, card));
    }
    trigger.into_any_element()
}

/// The interface font pop-up (Pink appearance.rs `render_font_picker`): a
/// 200 select with the family in use; its 240 menu holds a filter field
/// over the families (prefix matches first), the one in use checked, the
/// highlight shared by arrows and pointer, families this device lacks
/// listed at 45% and skipped.
#[allow(clippy::too_many_arguments)]
fn font_picker(
    f: &Frame,
    fonts: &Rc<Vec<FontFamily>>,
    font: &SharedString,
    cursor: Option<usize>,
    visible: &[usize],
    query: &SharedString,
    search: &Entity<TextInput>,
    focus: &FocusHandle,
) -> AnyElement {
    let t = f.t;
    let open = f.menu == Some(Menu::Font);
    let id = SharedString::from(format!("{}-font", f.key));
    let click_state = f.state.clone();
    let key_state = f.state.clone();
    let mut trigger = pink::select_frame(id.clone(), ControlSize::Md, open, true, t)
        .aria_label(format!("Interface font family: {font}"))
        .relative()
        .w(px(FAMILY_PICKER_WIDTH))
        .track_focus(focus)
        // Closed, any list key opens the menu and the filter takes over.
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            if matches!(key, "up" | "down" | "left" | "right" | "home" | "end") {
                key_state.update(cx, |s, cx| {
                    if s.menu != Some(Menu::Font) {
                        s.dismissed_at = None;
                        s.toggle_font(window, cx);
                    }
                });
                cx.stop_propagation();
            } else if matches!(key, "enter" | "space") {
                cx.stop_propagation();
            }
        })
        .on_click(move |_, window, cx| {
            click_state.update(cx, |s, cx| {
                window.focus(&s.font_focus, cx);
                s.toggle_font(window, cx);
            })
        })
        .child(div().flex_1().min_w_0().truncate().child(font.clone()))
        .child(pink::select_chevron(t));
    if let Some(exit) = f.exit(Menu::Font) {
        let rows: Vec<AnyElement> = visible
            .iter()
            .map(|&ix| {
                let family = &fonts[ix];
                let active = &family.label == font;
                let highlighted = cursor == Some(ix);
                let available = family.available;
                let move_state = f.state.clone();
                let click_state = f.state.clone();
                pink::menu_row_nav(active, highlighted, t)
                    .id(SharedString::from(format!("{id}-option-{ix}")))
                    .role(Role::Button)
                    .aria_label(format!("Interface font family: {}", family.label))
                    .when(available, |row| {
                        row.on_mouse_move(move |_: &MouseMoveEvent, _, cx| {
                            move_state.update(cx, |s, cx| {
                                if s.font_cursor != Some(ix) {
                                    s.font_cursor = Some(ix);
                                    cx.notify();
                                }
                            })
                        })
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            apply(&click_state, window, cx, |s, window, cx| {
                                s.font_cursor = Some(ix);
                                s.commit_font(ix, window, cx)
                            })
                        })
                    })
                    .when(!available, |row| row.opacity(0.45).cursor_default())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(family.label.clone()),
                    )
                    .into_any_element()
            })
            .collect();
        let list: AnyElement = if rows.is_empty() {
            div()
                .px(px(8.0))
                .py(px(6.0))
                .type_style(TextStyle::Footnote)
                .text_color(t.readable_muted(0.85))
                .child(if query.trim().is_empty() {
                    "No fonts"
                } else {
                    "No matching fonts"
                })
                .into_any_element()
        } else {
            div()
                .id(SharedString::from(format!("{id}-list")))
                .max_h(px(FAMILY_MENU_MAX_HEIGHT))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(layout::MENU_GAP))
                .children(rows)
                .into_any_element()
        };
        let close_state = f.state.clone();
        let card = pink::popover_card(t)
            .id(SharedString::from(format!("{id}-card")))
            .w(px(FAMILY_MENU_WIDTH))
            .flex()
            .flex_col()
            .on_mouse_down_out(move |_, _, cx| {
                close_state.update(cx, |s, cx| s.dismiss(Menu::Font, cx))
            })
            .child(
                pink::search_input_frame(t)
                    .line_height(px(18.0))
                    .child(search.clone()),
            )
            .child(list);
        trigger = trigger.child(anchored_menu_below(format!("{id}-menu").into(), exit, card));
    }
    trigger.into_any_element()
}

/// The interface size pop-up (Pink appearance.rs `render_size_picker`): a
/// 92 select reading "16 px"; its 112 menu lists the ladder with the size
/// in use checked. Arrows, Home and End open it and move the highlight
/// from the closed trigger; Enter commits, Escape closes.
fn size_picker(f: &Frame, size: usize, cursor: usize, focus: &FocusHandle) -> AnyElement {
    let t = f.t;
    let open = f.menu == Some(Menu::Size);
    let id = SharedString::from(format!("{}-size", f.key));
    let label = format!("{} px", INTERFACE_SIZES[size]);
    let key_state = f.state.clone();
    let click_state = f.state.clone();
    let mut trigger = pink::select_frame(id.clone(), ControlSize::Md, open, true, t)
        .aria_label(format!("Interface font size: {label}"))
        .relative()
        .w(px(SIZE_PICKER_WIDTH))
        .track_focus(focus)
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let last = INTERFACE_SIZES.len() - 1;
            let key = event.keystroke.key.as_str();
            let target = |s: &PickerState| match key {
                "up" | "left" => Some(s.size_cursor.saturating_sub(1)),
                "down" | "right" => Some((s.size_cursor + 1).min(last)),
                "home" => Some(0),
                "end" => Some(last),
                _ => None,
            };
            match key {
                "up" | "left" | "down" | "right" | "home" | "end" => {
                    key_state.update(cx, |s, cx| {
                        if s.menu != Some(Menu::Size) {
                            s.dismissed_at = None;
                            s.toggle_size(cx);
                        }
                        if let Some(next) = target(s) {
                            s.size_cursor = next;
                        }
                        cx.notify();
                    });
                    cx.stop_propagation();
                    window.prevent_default();
                }
                // Activation rides the keyboard click (Pink
                // `activate_size_from_keyboard`).
                "enter" | "space" => cx.stop_propagation(),
                "escape" => key_state.update(cx, |s, cx| {
                    if s.menu == Some(Menu::Size) {
                        s.size_cursor = s.size;
                        s.close(cx);
                        cx.stop_propagation();
                    }
                }),
                _ => {}
            }
        })
        .on_click(move |event: &ClickEvent, window, cx| {
            if event.is_keyboard() {
                apply(&click_state, window, cx, |s, _, cx| {
                    if s.menu == Some(Menu::Size) {
                        s.commit_size(cx)
                    } else {
                        s.dismissed_at = None;
                        s.toggle_size(cx);
                        None
                    }
                });
                return;
            }
            click_state.update(cx, |s, cx| {
                window.focus(&s.size_focus, cx);
                s.toggle_size(cx);
            })
        })
        .child(div().flex_1().min_w_0().truncate().child(label))
        .child(pink::select_chevron(t));
    if let Some(exit) = f.exit(Menu::Size) {
        let rows: Vec<AnyElement> = INTERFACE_SIZES
            .iter()
            .enumerate()
            .map(|(ix, px_size)| {
                let move_state = f.state.clone();
                let click_state = f.state.clone();
                pink::menu_row_nav(ix == size, ix == cursor, t)
                    .id(SharedString::from(format!("{id}-option-{ix}")))
                    .role(Role::Button)
                    .aria_label(format!("Interface font size: {px_size} px"))
                    .on_mouse_move(move |_: &MouseMoveEvent, _, cx| {
                        move_state.update(cx, |s, cx| {
                            if s.size_cursor != ix {
                                s.size_cursor = ix;
                                cx.notify();
                            }
                        })
                    })
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        apply(&click_state, window, cx, |s, _, cx| {
                            s.size_cursor = ix;
                            s.commit_size(cx)
                        })
                    })
                    .child(div().flex_1().child(format!("{px_size} px")))
                    .into_any_element()
            })
            .collect();
        let close_state = f.state.clone();
        let card = pink::popover_card(t)
            .id(SharedString::from(format!("{id}-card")))
            .w(px(SIZE_MENU_WIDTH))
            .flex()
            .flex_col()
            .on_mouse_down_out(move |_, _, cx| {
                close_state.update(cx, |s, cx| s.dismiss(Menu::Size, cx))
            })
            .children(rows);
        trigger = trigger.child(anchored_menu_below(format!("{id}-menu").into(), exit, card));
    }
    trigger.into_any_element()
}

/// The conversation width control (Pink appearance.rs
/// `render_width_control`): a 196 slider, 28 tall, whose 4 track in
/// `border` fills with the accent up to a 14 accent knob; the value as
/// footnote tabular figures in a 56 column; a small reset, enabled only
/// away from 736 px. Pointer presses and drags set it, arrows step by 16,
/// Home and End go to the ends.
fn width_control(
    f: &Frame,
    width: f32,
    dragging: bool,
    focus: &FocusHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
) -> AnyElement {
    let t = f.t;
    let fraction = ((width - CONVERSATION_WIDTH_MIN)
        / (CONVERSATION_WIDTH_MAX - CONVERSATION_WIDTH_MIN))
        .clamp(0.0, 1.0);
    let height = ControlSize::Md.height();
    let half = WIDTH_KNOB / 2.0;
    let ring_color = t.focus_ring();
    let press_state = f.state.clone();
    let key_state = f.state.clone();
    let record = bounds.clone();
    // While the pointer is down, the window's moves drive the value until
    // release.
    let drag = dragging.then(|| {
        let state = f.state.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let move_state = state.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble || !event.dragging() {
                        return;
                    }
                    let x = event.position.x;
                    apply(&move_state, window, cx, |s, _, _| {
                        s.width_at(x).and_then(|w| s.set_width(w))
                    });
                });
                let up_state = state.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    up_state.update(cx, |s, cx| {
                        s.dragging = false;
                        cx.notify();
                    });
                    window.refresh();
                });
            },
        )
        .absolute()
        .size_0()
    });
    let slider = div()
        .id(SharedString::from(format!("{}-width-slider", f.key)))
        .role(Role::Slider)
        .aria_label("Conversation width")
        .tab_index(0)
        .track_focus(focus)
        .relative()
        .flex_none()
        .w(px(WIDTH_SLIDER_WIDTH))
        .h(px(height))
        .rounded(px(6.0))
        .cursor_pointer()
        .focus_visible(move |style| pink::focus_ring(style, ring_color))
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| {
                let x = event.position.x;
                apply(&press_state, window, cx, |s, window, cx| {
                    window.focus(&s.slider_focus, cx);
                    s.dragging = true;
                    s.width_at(x).and_then(|w| s.set_width(w))
                });
                cx.stop_propagation();
            },
        )
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.clone();
            let step = match key.as_str() {
                "left" | "down" => Some(-CONVERSATION_WIDTH_STEP),
                "right" | "up" => Some(CONVERSATION_WIDTH_STEP),
                _ => None,
            };
            let to = match key.as_str() {
                "home" => Some(CONVERSATION_WIDTH_MIN),
                "end" => Some(CONVERSATION_WIDTH_MAX),
                _ => None,
            };
            if step.is_none() && to.is_none() {
                return;
            }
            cx.stop_propagation();
            window.prevent_default();
            apply(&key_state, window, cx, |s, _, _| {
                let next = to.unwrap_or(s.width + step.unwrap_or(0.0));
                s.set_width(next)
            });
        })
        .child(
            canvas(move |rect, _, _| record.set(Some(rect)), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(
            div()
                .absolute()
                .left(px(half))
                .right(px(half))
                .top(px((height - WIDTH_TRACK) / 2.0))
                .h(px(WIDTH_TRACK))
                .rounded_full()
                .bg(t.border)
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(fraction))
                        .rounded_full()
                        .bg(f.accent.primary),
                )
                .child(
                    div()
                        .absolute()
                        .left(gpui::relative(fraction))
                        .ml(px(-half))
                        .top(px((WIDTH_TRACK - WIDTH_KNOB) / 2.0))
                        .size(px(WIDTH_KNOB))
                        .rounded_full()
                        .bg(f.accent.primary),
                ),
        )
        .children(drag);
    let at_default = (width - CONVERSATION_WIDTH_DEFAULT).abs() < 0.5;
    let reset_state = f.state.clone();
    let reset = pink::icon_button_enabled(
        format!("{}-width-reset", f.key),
        ICON_RESTART,
        ControlSize::Sm,
        !at_default,
        t,
    )
    .aria_label("Reset conversation width")
    .tooltip(pink::text_tooltip(format!(
        "Reset to {CONVERSATION_WIDTH_DEFAULT:.0} px"
    )))
    .tooltip_show_delay(layout::TOOLTIP_DELAY)
    .when(!at_default, |reset| {
        reset.on_click(move |_, window, cx| {
            apply(&reset_state, window, cx, |s, _, _| {
                s.set_width(CONVERSATION_WIDTH_DEFAULT)
            })
        })
    });
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(slider)
        .child(
            div()
                .flex_none()
                .w(px(WIDTH_VALUE_W))
                .text_right()
                .type_style(TextStyle::Footnote)
                .font_features(pink::tabular_figures())
                .text_color(t.readable_muted(0.9))
                .child(format!("{width:.0} px")),
        )
        .child(reset)
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

pub fn story(_window: &mut Window, cx: &mut App) -> gpui::AnyView {
    story::view(cx, |_, _| {
        AppearancePicker::new("appearance-picker")
            .mode(AppearanceMode::System)
            .dark_theme("pink-dark")
            .light_theme("pink-light")
            .accent(AccentChoice::ThemeDefault)
            .surface(SurfacePreference::ThemeDefault)
            .fonts([
                FontFamily::new("Geist"),
                FontFamily::new("Geist Mono"),
                FontFamily::new("System UI"),
                FontFamily::new("Avenir Next"),
                FontFamily::new("Berkeley Mono").unavailable(),
                FontFamily::new("Helvetica Neue"),
                FontFamily::new("IBM Plex Sans"),
                FontFamily::new("Inter"),
                FontFamily::new("JetBrains Mono"),
                FontFamily::new("Menlo"),
                FontFamily::new("SF Pro"),
            ])
            .font("Geist")
            .interface_size(16)
            .conversation_width(832.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_snaps_to_the_step_and_range() {
        assert_eq!(normalize_width(736.0), 736.0);
        assert_eq!(normalize_width(741.0), 736.0);
        assert_eq!(normalize_width(745.0), 752.0);
        assert_eq!(normalize_width(10.0), CONVERSATION_WIDTH_MIN);
        assert_eq!(normalize_width(5000.0), CONVERSATION_WIDTH_MAX);
        assert_eq!(normalize_width(f32::NAN), CONVERSATION_WIDTH_DEFAULT);
    }

    #[test]
    fn sizes_snap_to_the_ladder() {
        assert_eq!(size_ix(16), 4);
        assert_eq!(size_ix(17), 4);
        assert_eq!(size_ix(19), 5);
        assert_eq!(size_ix(40), 6);
        assert_eq!(size_ix(0), 0);
    }

    #[test]
    fn accents_follow_pinks_derivation() {
        let violet = AccentChoice::Violet.color(true).unwrap();
        let roles = Accent::derive(violet, true, Rgb::hex(0x060606));
        // The kit's verified glyph rows for Violet on Pink Dark.
        assert_eq!(roles.glyph[0], Rgb::hex(0xaba1f9).hsla());
        assert_eq!(roles.glyph[1], Rgb::hex(0x8b7cf6).hsla());
        assert_eq!(roles.glyph[2], Rgb::hex(0x7266ca).hsla());
        // Ayu Light's orange sits below 3:1 on its page and is darkened.
        let ayu = Rgb::hex(0xf29718);
        assert!(ayu.contrast(Rgb::hex(0xfcfcfc)) < 3.0);
        let held = ayu.ensure_contrast(Rgb::hex(0xfcfcfc), 3.0);
        assert!(held.contrast(Rgb::hex(0xfcfcfc)) >= 3.0);
    }

    #[test]
    fn fonts_filter_by_prefix_then_substring() {
        let fonts: Vec<FontFamily> = ["Geist", "Geist Mono", "Inter", "IBM Plex Mono"]
            .into_iter()
            .map(FontFamily::new)
            .collect();
        assert_eq!(filter_fonts("", &fonts), vec![0, 1, 2, 3]);
        assert_eq!(filter_fonts("mono", &fonts), vec![1, 3]);
        assert_eq!(filter_fonts("i", &fonts), vec![2, 3, 0, 1]);
    }

    #[test]
    fn font_steps_skip_missing_families_and_stop_at_the_ends() {
        let fonts = vec![
            FontFamily::new("A"),
            FontFamily::new("B").unavailable(),
            FontFamily::new("C"),
        ];
        let visible = vec![0, 1, 2];
        assert_eq!(step_font(Some(0), 1, &visible, &fonts), Some(2));
        assert_eq!(step_font(Some(2), 1, &visible, &fonts), Some(2));
        assert_eq!(step_font(Some(2), -1, &visible, &fonts), Some(0));
        assert_eq!(step_font(Some(0), -1, &visible, &fonts), Some(0));
    }

    #[test]
    fn the_registry_lists_both_appearances_in_order() {
        let themes = ThemeVariant::builtins();
        assert_eq!(themes.iter().filter(|v| !v.dark).count(), 10);
        assert_eq!(themes.iter().filter(|v| v.dark).count(), 20);
        assert_eq!(themes[0].id.as_ref(), "pink-light");
        assert!(themes[0].frosted && themes[1].frosted);
    }
}
