// PINK's first-run welcome (empty_project_moon.rs and the onboarding canvas
// in shell.rs): a lit moon drawn in text over one question and the one
// action that starts a project. The moon is a sphere rendered into a
// `COLUMNS` x `ROWS` grid of block glyphs: the near side from PINK's
// NASA-derived artwork (assets/moon-full-ascii.txt), the far side
// procedural. Only its surface turns, once, over `TURN_SECONDS`; light
// appearance inks it like paper, dark appearance lights it.
//
// The package draws only. The host owns the clock (when a turn started,
// reduced motion, animation frames), the resolved colours and mono font,
// the action button with its id, role, accessible name and listener, and
// the moon's role, accessible name and hover listener. The question and
// the action's type are rems, so they follow the host's Interface size;
// the moon is sized in pixels from the space it has. Plain `//` comments:
// the pink-ui gallery compiles this file in place with `include!` for its
// story.

use std::{
    f32::consts::{PI, TAU},
    sync::OnceLock,
};

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The text grid.
pub const COLUMNS: usize = 72;
pub const ROWS: usize = 39;
/// The albedo map wrapped round the sphere.
const TEXTURE_WIDTH: usize = 144;
const TEXTURE_HEIGHT: usize = 72;
/// One turn of the surface, in seconds at normal motion speed.
pub const TURN_SECONDS: f32 = 4.8;
/// From empty space to full ink.
pub const GLYPHS: [char; 7] = [' ', '·', ':', '░', '▒', '▓', '█'];
/// The moon at full scale, and the share of it a small window keeps.
pub const MOON_DIAMETER: f32 = 320.0;
pub const MOON_MIN_SCALE: f32 = 0.58;
/// The question under the moon: the space above it, its size.
pub const HEADING_GAP: f32 = 22.0;
pub const HEADING_SIZE: f32 = 20.0;
/// The action under the question: the space above it, its padding and
/// text size.
pub const ACTION_GAP: f32 = 24.0;
pub const ACTION_PAD_X: f32 = 20.0;
pub const ACTION_PAD_Y: f32 = 10.0;
pub const ACTION_TEXT: f32 = 14.0;

/// PINK's near-side artwork.
const NEAR_SIDE: &str = include_str!("../assets/moon-full-ascii.txt");

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The moon's diameter for a column `width` wide in a window `height`
/// tall: full size from 900 tall and 440 wide, shrinking with the smaller
/// of the two down to `MOON_MIN_SCALE`.
pub fn moon_diameter(width: f32, height: f32) -> f32 {
    MOON_DIAMETER * (height / 900.0).min(width / 440.0).clamp(MOON_MIN_SCALE, 1.0)
}

/// Where a turn stands from its `progress` (0..1 of `TURN_SECONDS`):
/// smoothstep through one revolution, `None` once it has finished (and
/// for no turn at all), when the host parks the moon.
pub fn turn_at(progress: Option<f32>) -> Option<f32> {
    let p = progress.filter(|p| *p < 1.0)?.max(0.0);
    Some(p * p * (3.0 - 2.0 * p))
}

/// The grid's lines with the surface turned by `turn` (0..1 of a
/// revolution), inked for a light or a dark appearance.
pub fn moon_rows(turn: f32, light_appearance: bool) -> Vec<SharedString> {
    Surface::shared().rows(turn, light_appearance)
}

/// What the moon draws: its ink, the mono family and that family's advance
/// at 1 px (the host measures it with its text system), so the glyphs fill
/// the diameter exactly.
#[derive(Clone, Debug)]
pub struct MoonStyle {
    pub text: Hsla,
    pub font_family: SharedString,
    pub advance: f32,
}

/// The ink's strength: paper-grey in a light appearance; in a dark one it
/// brightens through a turn and settles back.
pub fn moon_ink(text: Hsla, turn: f32, light_appearance: bool) -> Hsla {
    text.opacity(if light_appearance {
        0.64
    } else {
        0.72 + 0.12 * (PI * turn).sin()
    })
}

/// The moon: `rows` (from [`moon_rows`]) in a `diameter` square, one line
/// per `ROWS`, the glyph size set so `COLUMNS` advances span the diameter.
pub fn moon(rows: Vec<SharedString>, diameter: f32, style: &MoonStyle) -> Div {
    let line_height = diameter / ROWS as f32;
    div()
        .size(px(diameter))
        .overflow_hidden()
        .font_family(style.font_family.clone())
        .text_size(px(diameter / (COLUMNS as f32 * style.advance.max(0.1))))
        .line_height(px(line_height))
        .text_color(style.text)
        .children(
            rows.into_iter()
                .map(move |line| div().h(px(line_height)).whitespace_nowrap().child(line)),
        )
}

/// The question under the moon: `HEADING_GAP` above it, `HEADING_SIZE`
/// semibold.
pub fn heading(color: Hsla, copy: impl Into<SharedString>) -> Div {
    div()
        .mt(px(HEADING_GAP))
        .text_size(ui_rems(HEADING_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(copy.into())
}

/// Sets the host's action button under the question: `ACTION_GAP` above
/// it, padded `ACTION_PAD_X` by `ACTION_PAD_Y`, its label at
/// `ACTION_TEXT`.
pub fn action<E: Styled>(button: E) -> E {
    button
        .mt(px(ACTION_GAP))
        .px(px(ACTION_PAD_X))
        .py(px(ACTION_PAD_Y))
        .text_size(ui_rems(ACTION_TEXT))
}

/// The group: the moon, the question and the action in one centred
/// column.
pub fn welcome() -> Div {
    div().flex().flex_col().items_center()
}

struct Sample {
    longitude: f32,
    latitude: f32,
    light: f32,
}

// The sphere's fixed cells and the surface texture that turns under them.
struct Surface {
    cells: Vec<Option<Sample>>,
    texture: Vec<f32>,
}

impl Surface {
    fn shared() -> &'static Self {
        static SURFACE: OnceLock<Surface> = OnceLock::new();
        SURFACE.get_or_init(Self::new)
    }

    // 46 seeded craters and two ripples of albedo on the far side, the
    // artwork blended in across the near side, and a light from the upper
    // left.
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

    // The grid's lines with the surface turned by `turn` (0..1 of a
    // revolution). On white paper, low ink coverage means brighter light.
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

