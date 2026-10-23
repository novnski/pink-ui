//! Light/dark mode.
//!
//! Every component is drawn in Pink's design language and follows the
//! active mode: [`crate::pink::Pink::get`] picks the dark or light token set.
//! The mode lives in a GPUI global; components read it with [`ActiveTheme`].

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

#[derive(Clone, Copy, Debug, Default)]
pub struct Theme {
    pub mode: ThemeMode,
}

impl Global for Theme {}

impl Theme {
    pub fn init(cx: &mut App) {
        if !cx.has_global::<Theme>() {
            cx.set_global(Theme::default());
        }
    }

    pub fn set_mode(mode: ThemeMode, cx: &mut App) {
        cx.set_global(Theme { mode });
        cx.refresh_windows();
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
