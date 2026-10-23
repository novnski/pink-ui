//! pink-ui: animated components for [GPUI](https://gpui.rs).
//!
//! Fifty-six components, one file each, drawn by GPUI natively and in the
//! browser through its web platform. Core components come in two looks, Clean
//! and Classic; pink components are drawn in pink's design language. Both
//! follow light and dark mode. Call [`init`] once at startup, then render any
//! component; [`components::COMPONENTS`] lists them all with a demo story.
//!
//! Components are builders you drop into any element tree, the same way
//! pink's controls work:
//!
//! ```no_run
//! use pink_ui::components::profile_menu::{ProfileMenu, ProfileSetup};
//! use gpui::{App, Context, Window, WindowOptions, div, prelude::*};
//!
//! struct Root;
//!
//! impl Render for Root {
//!     fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         pink_ui::motion::frame(window, cx); // eased hovers, once per frame
//!         div()
//!             .size_full()
//!             .child(
//!                 ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
//!                     .setup(ProfileSetup::new(3, 5).next("Add a profile photo")),
//!             )
//!             .child(pink_ui::motion::driver())
//!     }
//! }
//!
//! gpui_platform::application().with_assets(pink_ui::Assets).run(|cx: &mut App| {
//!     pink_ui::init(cx);
//!     cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Root)).unwrap();
//! });
//! ```

pub mod assets;
pub mod components;
pub mod interaction;
pub mod motion;
pub mod pink;
pub mod primitives;
pub mod story;
pub mod theme;

pub use assets::Assets;
pub use interaction::HoverFill;
pub use theme::{ActiveTheme, Look, Theme, ThemeMode};

/// Sets up the theme global, the bundled fonts and the text input key bindings.
pub fn init(cx: &mut gpui::App) {
    theme::Theme::init(cx);
    assets::load_fonts(cx);
    primitives::text_input::bind_keys(cx);
}
