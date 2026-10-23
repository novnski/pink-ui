//! Runs one component demo at a time in the browser.
//!
//! The showcase site loads this module in an iframe (`play.html`). GPUI's web
//! platform draws into a single canvas that fills the page, with WebGPU or a
//! WebGL2 fallback. The page's script calls [`start`] once, then [`show`],
//! [`set_theme`] and [`replay`] as the visitor clicks around, so
//! switching components never reloads the module. [`catalog`] needs no
//! browser: the site's build script calls it from Node.

#![cfg(target_family = "wasm")]

mod catalog;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    AnyView, App, ApplicationHandle, AsyncApp, Context, Pixels, Size, Window, WindowHandle,
    WindowOptions, div, prelude::*, px,
};
use pink_ui::{
    ActiveTheme, Theme, ThemeMode,
    components::COMPONENTS,
    primitives::{SANS, hex},
    story::Story,
};
use wasm_bindgen::prelude::*;

/// The running app. The handle keeps it alive; calls from the page go through
/// the async context, which (unlike the handle) flushes GPUI's effects after
/// each update, so a theme change actually redraws.
struct Running {
    _handle: ApplicationHandle,
    cx: AsyncApp,
}

thread_local! {
    static APP: RefCell<Option<Running>> = const { RefCell::new(None) };
    static PLAYER: RefCell<Option<WindowHandle<Player>>> = const { RefCell::new(None) };
}

/// The window root: the current story on the site's preview color.
struct Player {
    selected: usize,
    story: AnyView,
    /// Told the demo's size whenever it changes, so the page can fit the
    /// stage to it.
    on_resize: js_sys::Function,
    reported: Size<Pixels>,
    /// The stage's size, laid out in the same frame as the demo's.
    stage: Rc<Cell<Size<Pixels>>>,
}

impl Player {
    fn show(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = ix;
        // A fresh view starts the demo over, like the gallery's Replay.
        self.story = (COMPONENTS[ix].story)(window, cx);
        self.reported = Size::default();
        cx.notify();
    }

    /// Passes on the size the story measured in the last frame.
    fn report_size(&mut self, cx: &App) {
        let Ok(story) = self.story.clone().downcast::<Story>() else {
            return;
        };
        let size = story.read(cx).content_size();
        // A demo that fills the stage (laid out with `size_full`) grows with
        // it; reporting it would have the page grow the stage forever.
        let fills = (size.height - self.stage.get().height).abs() < px(0.5);
        if size != self.reported && size.height > Pixels::ZERO && !fills {
            self.reported = size;
            let (w, h) = (f32::from(size.width), f32::from(size.height));
            self.on_resize
                .call2(&JsValue::NULL, &w.into(), &h.into())
                .ok();
        }
    }
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        pink_ui::motion::frame(window, cx);
        self.report_size(cx);
        // The gallery's stage, and `--stage` in site/styles.css.
        let stage = if cx.theme().mode.is_dark() {
            hex(0x1f1f21)
        } else {
            hex(0xfafafa)
        };
        let stage_size = self.stage.clone();
        div()
            .size_full()
            .bg(stage)
            .font_family(SANS)
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(story) = bounds.first() {
                    stage_size.set(story.size);
                }
            })
            .child(self.story.clone())
    }
}

fn index_of(slug: &str) -> Option<usize> {
    COMPONENTS.iter().position(|c| c.slug == slug)
}

fn mode(dark: bool) -> ThemeMode {
    if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    }
}

/// Runs `f` on the app and the player window, if both are up.
fn with_player(f: impl FnOnce(&mut Player, &mut Window, &mut Context<Player>)) {
    let Some(window) = PLAYER.with(|p| *p.borrow()) else {
        return;
    };
    with_app(|cx| {
        window.update(cx, f).ok();
    });
}

fn with_app(f: impl FnOnce(&mut App)) {
    APP.with(|app| {
        if let Some(app) = app.borrow().as_ref() {
            app.cx.update(f);
        }
    });
}

/// Starts GPUI on the page's canvas with the component `slug`.
///
/// `on_ready` is called once graphics are up and the first story is mounted;
/// if the browser has neither WebGPU nor WebGL2 it is never called and GPUI
/// shows its own notice instead. `on_resize(width, height)` reports the
/// demo's laid-out size in CSS pixels whenever it changes.
#[wasm_bindgen]
pub fn start(slug: &str, dark: bool, on_ready: js_sys::Function, on_resize: js_sys::Function) {
    gpui_platform::web_init();
    let selected = index_of(slug).unwrap_or(0);
    let handle = gpui_platform::single_threaded_web()
        .with_assets(pink_ui::Assets)
        .run_embedded(move |cx: &mut App| {
            pink_ui::init(cx);
            Theme::set_mode(mode(dark), cx);
            let window = cx
                .open_window(WindowOptions::default(), |window, cx| {
                    let story = (COMPONENTS[selected].story)(window, cx);
                    cx.new(|_| Player {
                        selected,
                        story,
                        on_resize,
                        reported: Size::default(),
                        stage: Rc::default(),
                    })
                })
                .expect("failed to open the player window");
            PLAYER.with(|p| *p.borrow_mut() = Some(window));
            on_ready.call0(&JsValue::NULL).ok();
        });
    let cx = handle.to_async();
    APP.with(|app| {
        *app.borrow_mut() = Some(Running {
            _handle: handle,
            cx,
        })
    });
}

/// Shows the component `slug`, starting its demo from the beginning.
/// Returns false for an unknown slug.
#[wasm_bindgen]
pub fn show(slug: &str) -> bool {
    let Some(ix) = index_of(slug) else {
        return false;
    };
    with_player(|player, window, cx| player.show(ix, window, cx));
    true
}

/// Restarts the current demo.
#[wasm_bindgen]
pub fn replay() {
    with_player(|player, window, cx| player.show(player.selected, window, cx));
}

#[wasm_bindgen]
pub fn set_theme(dark: bool) {
    with_app(|cx| Theme::set_mode(mode(dark), cx));
}

/// Every component's slug, name, category, description and source
/// file, as JSON.
#[wasm_bindgen]
pub fn catalog() -> String {
    catalog::json()
}
