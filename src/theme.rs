//! Light/dark mode and the Clean/Classic look.
//!
//! Every core component carries its own color tokens, with a light set and a
//! dark override. Each one also comes in two looks: **Clean**, one flat card
//! whose edges are hairline shadows (the default), and **Classic**, white
//! cards on a gray tray. The active mode and look live in a GPUI global;
//! components read them with [`ActiveTheme`] and can override the look per
//! instance.

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

/// The two looks of the core components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    /// One flat card; strokes are shadows, like a popover.
    #[default]
    Clean,
    /// Cards on a gray tray.
    Classic,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Theme {
    pub mode: ThemeMode,
    pub look: Look,
}

impl Global for Theme {}

impl Theme {
    pub fn init(cx: &mut App) {
        if !cx.has_global::<Theme>() {
            cx.set_global(Theme::default());
        }
    }

    pub fn set_mode(mode: ThemeMode, cx: &mut App) {
        let look = cx.theme().look;
        cx.set_global(Theme { mode, look });
        cx.refresh_windows();
    }

    pub fn set_look(look: Look, cx: &mut App) {
        let mode = cx.theme().mode;
        cx.set_global(Theme { mode, look });
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

    /// The global look; components take `look.unwrap_or(cx.look())` so a
    /// per-instance `.look(..)` wins.
    fn look(&self) -> Look {
        self.theme().look
    }
}

impl ActiveTheme for App {
    fn theme(&self) -> Theme {
        self.try_global::<Theme>().copied().unwrap_or_default()
    }
}
