//! Welcome Screen – Pink's first-run screen: what the main column shows
//! before there is a project to work in.
//!
//! A lit moon drawn in text sits over one question and one way forward. The
//! moon is a sphere rendered into a 72 × 39 grid of block glyphs (the
//! near side from Pink's NASA-derived artwork, the far side procedural);
//! under the pointer only its surface turns, once, over 4.8 s, and then it
//! parks. Light mode inks it like paper, dark mode lights it. The whole
//! group arrives with Pink's 500 ms rise.
//!
//! ```ignore
//! WelcomeScreen::new("welcome")
//!     .size(940.0, 800.0)
//!     .on_add_project(|_, _| println!("pick a folder"))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyView, App, Context, ElementId, Font, IntoElement, Role, SharedString, Task, Window, div,
    prelude::*, px,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink},
    story,
};

pub const NAME: &str = "Welcome Screen";
pub const DESCRIPTION: &str = "The first thing a new workspace shows: a lit text-drawn moon that turns once under the pointer, above one question and the one button that starts a project";

/// The `pink-welcome-screen` package (`components/welcome-screen`): PINK's
/// text-drawn moon (empty_project_moon.rs), its question and the action's
/// spacing, compiled here unchanged. The story adds the canvas, the clock
/// and the kit's Primary button.
#[allow(dead_code)]
pub mod package {
    include!("../../components/welcome-screen/src/lib.rs");
}
use package::{TURN_SECONDS, moon_diameter, moon_rows, turn_at};

/// Pink: workbench/render.rs `CANVAS_RADIUS` – the canvas the screen sits on.
const CANVAS_RADIUS: f32 = 14.0;
/// The demo turns the moon itself this often (Pink waits for the pointer).
const AUTOPLAY_FIRST: Duration = Duration::from_millis(900);
const AUTOPLAY_EVERY: Duration = Duration::from_secs(9);

/// The moon's state (Pink `AsciiMoon`): when the turn started and the last
/// lines drawn.
struct MoonState {
    started: Option<Instant>,
    cached_turn: f32,
    cached_light: bool,
    rows: Vec<SharedString>,
    _autoplay: Option<Task<()>>,
}

impl MoonState {
    fn new(autoplay: bool, cx: &mut Context<Self>) -> Self {
        let autoplay = autoplay.then(|| {
            cx.spawn(async move |this, cx| {
                let mut wait = AUTOPLAY_FIRST;
                loop {
                    cx.background_executor().timer(wait).await;
                    wait = AUTOPLAY_EVERY;
                    let turned = this.update(cx, |m, cx| {
                        if m.started.is_none() && !motion::reduced_motion(cx) {
                            m.started = Some(Instant::now());
                            cx.notify();
                        }
                    });
                    if turned.is_err() {
                        break;
                    }
                }
            })
        });
        Self {
            started: None,
            cached_turn: 0.0,
            cached_light: false,
            rows: moon_rows(0.0, false),
            _autoplay: autoplay,
        }
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

type AddHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct WelcomeScreen {
    id: ElementId,
    heading: SharedString,
    action: SharedString,
    width: f32,
    height: f32,
    diameter: Option<f32>,
    autoplay: bool,
    on_add_project: Option<AddHandler>,
}

impl WelcomeScreen {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            heading: "What will you build?".into(),
            action: "Add project".into(),
            width: 940.0,
            height: 800.0,
            diameter: None,
            autoplay: false,
            on_add_project: None,
        }
    }

    /// The question under the moon.
    pub fn heading(mut self, heading: impl Into<SharedString>) -> Self {
        self.heading = heading.into();
        self
    }

    /// The button's label.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action = label.into();
        self
    }

    /// The canvas's size; the moon scales with it like Pink's.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Draws the moon at this diameter instead of scaling it.
    pub fn moon_diameter(mut self, diameter: f32) -> Self {
        self.diameter = Some(diameter);
        self
    }

    /// Turns the moon every few seconds on its own, not only under the
    /// pointer (for demos).
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    pub fn on_add_project(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_add_project = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for WelcomeScreen {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let autoplay = self.autoplay;
        let moon = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "moon".into()),
            cx,
            move |_, cx| MoonState::new(autoplay, cx),
        );
        let diameter = self
            .diameter
            .unwrap_or_else(|| moon_diameter(self.width, self.height));

        // Pink `AsciiMoon::render`: the turn runs on animation frames and
        // parks when it completes (or never starts under reduced motion).
        let reduced = motion::reduced_motion(cx);
        let light = !t.dark;
        let (turn, rows) = moon.update(cx, |m, _| {
            let duration = TURN_SECONDS * motion::speed_scale().max(0.01);
            let progress = m
                .started
                .map(|start| start.elapsed().as_secs_f32() / duration);
            let turn = match turn_at(progress).filter(|_| !reduced) {
                Some(turn) => {
                    window.request_animation_frame();
                    turn
                }
                None => {
                    m.started = None;
                    0.0
                }
            };
            if m.cached_turn != turn || m.cached_light != light {
                m.rows = moon_rows(turn, light);
                m.cached_turn = turn;
                m.cached_light = light;
            }
            (turn, m.rows.clone())
        });

        // The glyph grid fills the diameter exactly: Geist Mono's advance
        // sets the size, the rows split the height.
        let font = Font {
            family: pink::MONO.into(),
            ..Default::default()
        };
        let font_id = window.text_system().resolve_font(&font);
        let advance = window
            .text_system()
            .advance(font_id, px(1.0), '0')
            .map(|size| f32::from(size.width))
            .unwrap_or(0.6)
            .max(0.1);
        let hover_moon = moon.clone();
        let moon_el = package::moon(
            rows,
            diameter,
            &package::MoonStyle {
                text: package::moon_ink(t.text, turn, light),
                font_family: pink::MONO.into(),
                advance,
            },
        )
        .id(ElementId::Name(format!("{key}-moon").into()))
        .role(Role::Image)
        .aria_label("Moon")
        .on_hover(move |hovered: &bool, _, cx| {
            let hovered = *hovered;
            hover_moon.update(cx, |m, cx| {
                if hovered && m.started.is_none() && !motion::reduced_motion(cx) {
                    m.started = Some(Instant::now());
                    cx.notify();
                }
            });
        });

        // Pink: shell.rs – one clear affordance: the kit's Primary at the
        // dialog size, widened and set at 14.
        let on_add = self.on_add_project.clone();
        let add = pink::button(
            format!("{key}-add"),
            ButtonKind::Primary,
            ControlSize::Lg,
            &t,
        )
        .aria_label(self.action.clone())
        .child(self.action.clone())
        .on_click(move |_, window, cx| {
            if let Some(f) = on_add.as_ref() {
                f(window, cx);
            }
        });
        // The package sets it under the question at the dialog size.
        let add = package::action(add);

        let group = package::welcome()
            .child(moon_el)
            .child(package::heading(t.text, self.heading.clone()))
            .child(add);

        // The main column's canvas (Pink: workbench/render.rs
        // `contained_plane`): radius 14, a hairline, the canvas fill.
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .rounded(px(CANVAS_RADIUS))
            .border_1()
            .border_color(t.hairline(0.08))
            .bg(t.canvas())
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(motion::fade_in_rise(
                ElementId::Name(format!("{key}-canvas").into()),
                group,
            ))
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        WelcomeScreen::new("welcome-screen")
            .size(940.0, 800.0)
            .autoplay(true)
    })
}

#[cfg(test)]
mod tests {
    use super::package::{COLUMNS, GLYPHS, MOON_DIAMETER, ROWS};
    use super::*;

    #[test]
    fn the_moon_fills_its_grid() {
        let rows = moon_rows(0.0, false);
        assert_eq!(rows.len(), ROWS);
        assert!(rows.iter().all(|row| row.chars().count() <= COLUMNS));
        // The disc reaches the grid's sides on its middle row and leaves the
        // corners empty.
        let middle = &rows[ROWS / 2];
        assert!(middle.chars().count() >= COLUMNS - 3);
        assert!(rows[0].trim().len() < middle.trim().len());
        // Every lit cell is ink; empty space only outside the disc.
        assert!(middle.trim().chars().all(|c| GLYPHS[1..].contains(&c)));
    }

    #[test]
    fn light_mode_inks_like_paper() {
        // The light falls from the upper left: in dark mode that quadrant is
        // the dense one, on paper it is the sparse one.
        let weight = |rows: &[SharedString], upper_left: bool| -> usize {
            rows.iter()
                .enumerate()
                .filter(|(row, _)| (*row < ROWS / 2) == upper_left)
                .flat_map(|(_, line)| {
                    line.chars()
                        .enumerate()
                        .filter(|(col, _)| (*col < COLUMNS / 2) == upper_left)
                        .map(|(_, c)| GLYPHS.iter().position(|g| *g == c).unwrap_or(0))
                        .collect::<Vec<_>>()
                })
                .sum()
        };
        let dark = moon_rows(0.0, false);
        let light = moon_rows(0.0, true);
        assert_ne!(dark, light);
        assert!(weight(&dark, true) > weight(&dark, false));
        assert!(weight(&light, true) < weight(&light, false));
    }

    #[test]
    fn a_turn_moves_the_surface_and_comes_back() {
        let rest = moon_rows(0.0, false);
        assert_ne!(moon_rows(0.25, false), rest);
        assert_eq!(moon_rows(1.0, false), rest);
        assert_eq!(turn_at(None), None);
        assert_eq!(turn_at(Some(0.0)), Some(0.0));
        assert_eq!(turn_at(Some(0.5)), Some(0.5));
        assert_eq!(turn_at(Some(1.0)), None);
    }

    #[test]
    fn the_moon_scales_like_pinks() {
        assert_eq!(moon_diameter(1200.0, 1000.0), MOON_DIAMETER);
        assert_eq!(moon_diameter(940.0, 720.0), MOON_DIAMETER * 0.8);
        assert_eq!(moon_diameter(200.0, 300.0), MOON_DIAMETER * 0.58);
    }
}
