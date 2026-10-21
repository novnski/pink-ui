//! affectUI for GPUI.
//!
//! The components from <https://www.affectui.com/components>, rebuilt as
//! native GPUI elements: ports of affectUI's free components, and originals
//! in luna's design language built around the ideas of its Pro catalog.
//! Call [`init`] once at startup, then render any component;
//! [`components::COMPONENTS`] lists them all with a demo story.
//!
//! Components are builders you drop into any element tree, the same way
//! luna's controls work:
//!
//! ```no_run
//! use affectui::components::profile_menu::{ProfileMenu, ProfileSetup};
//! use gpui::{App, Application, Context, Window, WindowOptions, div, prelude::*};
//!
//! struct Root;
//!
//! impl Render for Root {
//!     fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//!         affectui::motion::frame(window, cx); // eased hovers, once per frame
//!         div()
//!             .size_full()
//!             .child(
//!                 ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
//!                     .setup(ProfileSetup::new(3, 5).next("Add a profile photo")),
//!             )
//!             .child(affectui::motion::driver())
//!     }
//! }
//!
//! Application::new().with_assets(affectui::Assets).run(|cx: &mut App| {
//!     affectui::init(cx);
//!     cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Root)).unwrap();
//! });
//! ```

pub mod assets;
pub mod components;
pub mod interaction;
pub mod luna;
pub mod motion;
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
