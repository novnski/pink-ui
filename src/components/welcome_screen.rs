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

use std::{
    f32::consts::{PI, TAU},
    rc::Rc,
    sync::{Arc, OnceLock},
};
use web_time::{Duration, Instant};

use gpui::{
    AnyView, App, Context, ElementId, Font, FontWeight, IntoElement, Role, SharedString, Task,
    Window, div, prelude::*, px,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink},
    story,
};

pub const NAME: &str = "Welcome Screen";
pub const DESCRIPTION: &str = "The first thing a new workspace shows – a lit text-drawn moon that turns once under the pointer, above one question and the one button that starts a project";

/// Pink: empty_project_moon.rs – the text grid.
const COLUMNS: usize = 72;
const ROWS: usize = 39;
/// Pink: empty_project_moon.rs – the albedo map wrapped round the sphere.
const TEXTURE_WIDTH: usize = 144;
const TEXTURE_HEIGHT: usize = 72;
/// Pink: empty_project_moon.rs `TURN_SECONDS` – one turn of the surface.
const TURN_SECONDS: f32 = 4.8;
/// Pink: empty_project_moon.rs `GLYPHS`, from empty space to full ink.
const GLYPHS: [char; 7] = [' ', '·', ':', '░', '▒', '▓', '█'];
/// Pink: shell.rs – the moon at full scale.
const MOON_DIAMETER: f32 = 320.0;
/// Pink: workbench/render.rs `CANVAS_RADIUS` – the canvas the screen sits on.
const CANVAS_RADIUS: f32 = 14.0;
/// The demo turns the moon itself this often (Pink waits for the pointer).
const AUTOPLAY_FIRST: Duration = Duration::from_millis(900);
const AUTOPLAY_EVERY: Duration = Duration::from_secs(9);

/// Pink's near-side artwork (`assets/illustrations/moon-full-ascii.txt`).
const NEAR_SIDE: &str = include_str!("../../assets/images/welcome-screen/moon-full-ascii.txt");

// ---------------------------------------------------------------------------
// The moon (Pink: empty_project_moon.rs `Surface`)
// ---------------------------------------------------------------------------

struct Sample {
    longitude: f32,
    latitude: f32,
    light: f32,
}

/// The sphere's fixed cells and the surface texture that turns under them.
struct Surface {
    cells: Vec<Option<Sample>>,
    texture: Vec<f32>,
}

impl Surface {
    fn shared() -> &'static Self {
        static SURFACE: OnceLock<Surface> = OnceLock::new();
        SURFACE.get_or_init(Self::new)
    }

    /// Pink `Surface::new`, value for value: 46 seeded craters and two
    /// ripples of albedo on the far side, the artwork blended in across the
    /// near side, and a light from the upper left.
    fn new() -> Self {
        let source: Vec<Vec<char>> = NEAR_SIDE
            .lines()
            .map(|line| line.chars().collect())
            .collect();
        let source_width = source.iter().map(Vec::len).max().unwrap_or(1);
        let mut seed = 38_u32;
        let mut random = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed as f32 / u32::MAX as f32
        };
        let craters: Vec<([f32; 3], f32)> = (0..46)
            .map(|_| {
                let y = random() * 1.8 - 0.9;
                let longitude = random() * TAU;
                let radius = (1.0 - y * y).sqrt();
                (
                    [radius * longitude.sin(), y, radius * longitude.cos()],
                    0.035 + random() * 0.13,
                )
            })
            .collect();
        let mut texture = Vec::with_capacity(TEXTURE_WIDTH * TEXTURE_HEIGHT);
        for row in 0..TEXTURE_HEIGHT {
            let latitude = (row as f32 + 0.5) / TEXTURE_HEIGHT as f32 * PI;
            for col in 0..TEXTURE_WIDTH {
                let longitude = col as f32 / TEXTURE_WIDTH as f32 * TAU;
                let x = longitude.sin() * latitude.sin();
                let y = -latitude.cos();
                let z = longitude.cos() * latitude.sin();
                let mut albedo = 0.72
                    + 0.08 * (19.0 * x + 7.0 * z).sin() * (22.0 * y - 9.0 * z).sin()
                    + 0.05 * (41.0 * x + 33.0 * y + 23.0 * z).sin();
                for (center, radius) in &craters {
                    let dot = x * center[0] + y * center[1] + z * center[2];
                    let distance = 2.0 * (1.0 - dot) / (radius * radius);
                    if distance < 2.0 {
                        albedo -= 0.17 * (-distance * 4.0).exp();
                        albedo += 0.10 * (-(distance - 0.8).powi(2) * 28.0).exp();
                    }
                }
                if z > 0.0 && !source.is_empty() {
                    let sx = (((x + 1.0) * 0.5 * (source_width - 1) as f32).round() as usize)
                        .min(source_width - 1);
                    let sy = (((y + 1.0) * 0.5 * (source.len() - 1) as f32).round() as usize)
                        .min(source.len() - 1);
                    let density = match source[sy].get(sx).copied().unwrap_or(' ') {
                        '█' => 1.0,
                        '▓' => 0.80,
                        '▒' => 0.56,
                        '░' => 0.30,
                        _ => 0.12,
                    };
                    let blend = (z / 0.3).clamp(0.0, 1.0);
                    let blend = blend * blend * (3.0 - 2.0 * blend);
                    albedo = albedo * (1.0 - blend) + (0.3 + density * 0.7) * blend;
                }
                texture.push(albedo.clamp(0.15, 1.0));
            }
        }
        let mut cells = Vec::with_capacity(COLUMNS * ROWS);
        for row in 0..ROWS {
            for col in 0..COLUMNS {
                let x = (col as f32 - (COLUMNS - 1) as f32 * 0.5) / (COLUMNS as f32 * 0.5 - 2.0);
                let y = (row as f32 - (ROWS - 1) as f32 * 0.5) / (ROWS as f32 * 0.5 - 1.0);
                let radius = x * x + y * y;
                cells.push((radius < 1.0).then(|| {
                    let z = (1.0 - radius).sqrt();
                    Sample {
                        longitude: x.atan2(z) / TAU,
                        latitude: (-y).acos() / PI,
                        light: 0.075 + ((-0.62 * x - 0.5 * y + z) / 1.279).max(0.0) * 0.94,
                    }
                }));
            }
        }
        Self { cells, texture }
    }

    /// The grid's lines with the surface turned by `turn` (0..1 of a
    /// revolution). On white paper, low ink coverage means brighter light.
    fn rows(&self, turn: f32, light_appearance: bool) -> Vec<SharedString> {
        self.cells
            .chunks(COLUMNS)
            .map(|row| {
                let text: String = row
                    .iter()
                    .map(|cell| {
                        let Some(sample) = cell else {
                            return ' ';
                        };
                        let u = (sample.longitude + turn).rem_euclid(1.0) * TEXTURE_WIDTH as f32;
                        let x = u.floor() as usize % TEXTURE_WIDTH;
                        let y = ((sample.latitude * TEXTURE_HEIGHT as f32) as usize)
                            .min(TEXTURE_HEIGHT - 1);
                        let next = (x + 1) % TEXTURE_WIDTH;
                        let a = self.texture[y * TEXTURE_WIDTH + x];
                        let b = self.texture[y * TEXTURE_WIDTH + next];
                        let value = (a + (b - a) * u.fract()) * sample.light;
                        let density = if light_appearance {
                            0.94 - value * 0.9
                        } else {
                            value
                        };
                        GLYPHS[(density * (GLYPHS.len() - 1) as f32)
                            .round()
                            .clamp(1.0, (GLYPHS.len() - 1) as f32)
                            as usize]
                    })
                    .collect();
                SharedString::from(text.trim_end().to_owned())
            })
            .collect()
    }
}

/// Pink: shell.rs – the moon scales with the window's height and the
/// column's width, from 58% to full size.
fn moon_diameter(width: f32, height: f32) -> f32 {
    MOON_DIAMETER * (height / 900.0).min(width / 440.0).clamp(0.58, 1.0)
}

/// Where the turn stands: smoothstep through one revolution, 0 at rest.
fn turn_at(progress: Option<f32>) -> Option<f32> {
    let p = progress.filter(|p| *p < 1.0)?;
    Some(p * p * (3.0 - 2.0 * p))
}

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
            rows: Surface::shared().rows(0.0, false),
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
                m.rows = Surface::shared().rows(turn, light);
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
        let line_height = diameter / ROWS as f32;
        let hover_moon = moon.clone();
        let moon_el = div()
            .id(ElementId::Name(format!("{key}-moon").into()))
            .role(Role::Image)
            .aria_label("Moon")
            .size(px(diameter))
            .overflow_hidden()
            .font_family(pink::MONO)
            .text_size(px(diameter / (COLUMNS as f32 * advance)))
            .line_height(px(line_height))
            .text_color(t.text.opacity(if light {
                0.64
            } else {
                0.72 + 0.12 * (PI * turn).sin()
            }))
            .children(
                rows.into_iter()
                    .map(|line| div().h(px(line_height)).whitespace_nowrap().child(line)),
            )
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
        .mt(px(24.0))
        .px(px(20.0))
        .py(px(10.0))
        .text_size(px(14.0))
        .child(self.action.clone())
        .on_click(move |_, window, cx| {
            if let Some(f) = on_add.as_ref() {
                f(window, cx);
            }
        });

        let group = div()
            .flex()
            .flex_col()
            .items_center()
            .child(moon_el)
            .child(
                div()
                    .mt(px(22.0))
                    .text_size(px(20.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(self.heading.clone()),
            )
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
    use super::*;

    #[test]
    fn the_moon_fills_its_grid() {
        let rows = Surface::shared().rows(0.0, false);
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
        let dark = Surface::shared().rows(0.0, false);
        let light = Surface::shared().rows(0.0, true);
        assert_ne!(dark, light);
        assert!(weight(&dark, true) > weight(&dark, false));
        assert!(weight(&light, true) < weight(&light, false));
    }

    #[test]
    fn a_turn_moves_the_surface_and_comes_back() {
        let rest = Surface::shared().rows(0.0, false);
        assert_ne!(Surface::shared().rows(0.25, false), rest);
        assert_eq!(Surface::shared().rows(1.0, false), rest);
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
