//! Embedded assets: the Inter font and every SVG icon under `assets/`.
//!
//! Components reference icons by path, e.g. `svg().path("icons/profile-menu/settings.svg")`.

use std::borrow::Cow;

use anyhow::Result;
use gpui::{App, AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "fonts/*.ttf"]
#[include = "icons/**/*.svg"]
#[include = "images/**/*"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|f| f.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|p| p.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

/// Registers the bundled Inter faces with the text system.
pub fn load_fonts(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = Assets::iter()
        .filter(|p| p.starts_with("fonts/") && p.ends_with(".ttf"))
        .filter_map(|p| Assets::get(&p).map(|f| f.data))
        .collect();
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("pink-ui: could not load bundled fonts: {err}");
    }
}
