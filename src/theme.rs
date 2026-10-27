//! Light/dark mode.
//!
//! Every component is drawn in Pink's design language and follows the
//! active mode: [`crate::pink::Pink::get`] picks the dark or light token set.
//! The mode lives in a GPUI global; components read it with [`ActiveTheme`].
//! The global also carries Pink's Interface size (`ui_font_size`, 16 by
//! default): geometry that Pink scales with it (the titlebar cluster, the
//! sidebar's rows and columns) reads [`crate::pink::ui_scale`].

use gpui::{App, Global};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

impl ThemeMode {
    pub fn is_dark(self) -> bool {
        self == ThemeMode::Dark
    }

    pub fn toggled(self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub mode: ThemeMode,
    /// Pink's Interface size in px (`Settings::ui_font_size`): 16 by
    /// default, between [`Theme::UI_FONT_SIZE_MIN`] and
    /// [`Theme::UI_FONT_SIZE_MAX`].
    pub ui_font_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            mode: ThemeMode::default(),
            ui_font_size: Theme::UI_FONT_SIZE_DEFAULT,
        }
    }
}

impl Global for Theme {}

impl Theme {
    pub fn init(cx: &mut App) {
        if !cx.has_global::<Theme>() {
            cx.set_global(Theme::default());
        }
    }

    /// Pink's Interface size steps (`typography::UiFontSize`): 12 to 20,
    /// 16 by default.
    pub const UI_FONT_SIZE_MIN: f32 = 12.0;
    pub const UI_FONT_SIZE_DEFAULT: f32 = 16.0;
    pub const UI_FONT_SIZE_MAX: f32 = 20.0;

    pub fn set_mode(mode: ThemeMode, cx: &mut App) {
        let theme = cx.theme();
        cx.set_global(Theme { mode, ..theme });
        cx.refresh_windows();
    }

    /// Set the Interface size (clamped to Pink's 12 to 20 px range).
    pub fn set_ui_font_size(size: f32, cx: &mut App) {
        let theme = cx.theme();
        cx.set_global(Theme {
            ui_font_size: size.clamp(Self::UI_FONT_SIZE_MIN, Self::UI_FONT_SIZE_MAX),
            ..theme
        });
        cx.refresh_windows();
    }

    /// Interface size over the 16 px baseline (Pink `Theme::ui_scale`).
    pub fn ui_scale(&self) -> f32 {
        self.ui_font_size / Self::UI_FONT_SIZE_DEFAULT
    }

    /// Picks the light or dark variant of a token.
    pub fn pick<T>(&self, light: T, dark: T) -> T {
        if self.mode.is_dark() { dark } else { light }
    }
}

pub trait ActiveTheme {
    fn theme(&self) -> Theme;

    fn is_dark(&self) -> bool {
        self.theme().mode.is_dark()
    }
}

impl ActiveTheme for App {
    fn theme(&self) -> Theme {
        self.try_global::<Theme>().copied().unwrap_or_default()
    }
}
