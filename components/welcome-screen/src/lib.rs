// PINK's first-run welcome (welcome_mark.rs and the onboarding canvas in
// shell.rs): a field of PINK's pink cells over one question, a line on what
// to do, the one action that starts a project and its shortcut.
//
// The field is the app icon grown into the room: rounded square cells on the
// icon's grid, lightest and largest at the heart (where the icon sits) and
// deepening, shrinking and fading as they spread out, until they dissolve
// well before the window's edges. Below the heart they fade faster, so the
// question and the action under it stay clear. When the field first appears
// its cells pop in from the centre outwards over `BUILD_SECONDS`; from then
// on pulses keep rolling out from the heart, one every half `PULSE_SECONDS`,
// lifting each ring of cells towards the highlight as they pass. The cells
// are painted quads, so the field looks the same in every font and size.
//
// The package draws only. The host owns the clocks (when the build and the
// pulse started, reduced motion, animation frames), the resolved colours and
// mono font, the action button with its id, role, accessible name and
// listener, and the field's role and accessible name. The text is rems, so
// it follows the host's Interface size; the field is sized in pixels from the
// space it has. Plain `//` comments: the pink-ui gallery compiles this file in
// place with `include!` for its story.

use gpui::{
    AbsoluteLength, Bounds, Div, FontWeight, Hsla, Rgba, SharedString, canvas, div, fill, point,
    prelude::*, px, rems, size,
};

/// The field at full scale: the room its heart takes in the column (where
/// the icon would sit), the pitch of its grid, the bounds on how far its
/// cells reach from the heart before they vanish (they reach for the side
/// and top edges of the room in between), and how much faster they fade
/// below the heart's row.
pub const HERO_SIDE: f32 = 168.0;
pub const FIELD_PITCH: f32 = 46.0;
pub const FIELD_REACH_MIN: f32 = 240.0;
pub const FIELD_REACH_MAX: f32 = 760.0;
pub const FIELD_DOWNWARD: f32 = 4.0;
/// How far the heart sits above the room's middle (half the words under
/// it), which leaves this much room up to the top edge.
pub const HEART_LIFT: f32 = 90.0;
/// The share of the field a small window keeps.
pub const FIELD_MIN_SCALE: f32 = 0.6;
/// A heart cell's side as a share of the pitch (the icon's 16 pt gaps on
/// its 824 pt tile), and how far the outermost cells shrink from it.
pub const CELL_SHARE: f32 = 0.86;
pub const CELL_SHRINK: f32 = 0.42;
/// A cell's corner radius, as a share of its side.
pub const CELL_ROUND: f32 = 0.22;
/// The distance (as a share of the reach) over which the shades run from
/// the heart's to the deepest; beyond it cells keep the deepest shade.
pub const SHADE_REACH: f32 = 0.55;
/// Where the fade starts, as a share of the reach.
pub const FADE_START: f32 = 0.12;
/// How far a passing pulse grows a cell, as a share of its side, and how
/// strongly it moves the cell towards the highlight.
pub const PULSE_GROW: f32 = 0.14;
pub const PULSE_LIGHT: f32 = 0.6;
/// The build when the field first appears, and the time one pulse takes to
/// roll from the heart to the reach (two are always on their way), in
/// seconds at normal motion speed.
pub const BUILD_SECONDS: f32 = 0.9;
pub const PULSE_SECONDS: f32 = 3.2;
/// The question under the heart: the space above it, its size.
pub const HEADING_GAP: f32 = 32.0;
pub const HEADING_SIZE: f32 = 22.0;
/// The line under the question: the space above it, its size, line height
/// and widest measure.
pub const SUBHEADING_GAP: f32 = 8.0;
pub const SUBHEADING_SIZE: f32 = 14.0;
pub const SUBHEADING_LINE: f32 = 20.0;
pub const SUBHEADING_WIDTH: f32 = 400.0;
/// The action under the line: the space above it, its padding and text
/// size.
pub const ACTION_GAP: f32 = 24.0;
pub const ACTION_PAD_X: f32 = 20.0;
pub const ACTION_PAD_Y: f32 = 10.0;
pub const ACTION_TEXT: f32 = 14.0;
/// The shortcut under the action: the space above it, its size, and the
/// keycap's padding and corners.
pub const HINT_GAP: f32 = 14.0;
pub const HINT_SIZE: f32 = 12.0;
pub const KEYCAP_PAD_X: f32 = 5.0;
pub const KEYCAP_PAD_Y: f32 = 1.0;
pub const KEYCAP_RADIUS: f32 = 4.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The field for a room `width` wide and `height` tall: its heart's side
/// and grid pitch (full size from 900 tall and 440 wide, shrinking with the
/// smaller of the two down to `FIELD_MIN_SCALE`), and how far it reaches
/// across (`reach_x`, to the side edges) and up (`reach_y`, to the top edge),
/// so it dissolves just as it meets the room's borders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldLayout {
    pub side: f32,
    pub pitch: f32,
    pub reach_x: f32,
    pub reach_y: f32,
}

pub fn field_layout(width: f32, height: f32) -> FieldLayout {
    let scale = (height / 900.0)
        .min(width / 440.0)
        .clamp(FIELD_MIN_SCALE, 1.0);
    let bound = |room: f32| room.clamp(FIELD_REACH_MIN * scale, FIELD_REACH_MAX * scale);
    FieldLayout {
        side: HERO_SIDE * scale,
        pitch: FIELD_PITCH * scale,
        reach_x: bound(width * 0.5),
        reach_y: bound(height * 0.5 - HEART_LIFT),
    }
}

/// Where a build stands from its `progress` (0..1 of `BUILD_SECONDS`), or
/// `None` once it has finished (and for no build at all).
pub fn build_at(progress: Option<f32>) -> Option<f32> {
    progress.filter(|p| *p < 1.0).map(|p| p.max(0.0))
}

/// The pulse's phase after `elapsed` seconds (scaled by the host's motion
/// speed): 0..1 through one `PULSE_SECONDS` cycle.
pub fn pulse_at(elapsed: f32) -> f32 {
    (elapsed / PULSE_SECONDS).rem_euclid(1.0)
}

/// One cell of the field: its centre as an offset from the heart (pixels),
/// its side at rest, its `distance` from the heart (0 at the heart to 1 at
/// the reach, counted faster below the heart's row), how far it has `appear`ed in the
/// build (0 to 1; 1 once built) and how much a passing pulse `lift`s it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldCell {
    pub x: f32,
    pub y: f32,
    pub extent: f32,
    pub distance: f32,
    pub appear: f32,
    pub lift: f32,
}

fn smoothstep(from: f32, to: f32, value: f32) -> f32 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much a pulse at `phase` lifts a cell at `distance`: a soft ring that
/// leaves the heart and dims as it travels. Two rings are always out, half
/// a cycle apart, so a new one leaves the heart every half `PULSE_SECONDS`.
pub fn pulse_lift(distance: f32, phase: f32) -> f32 {
    (0..2)
        .map(|wave| {
            let t = (phase + wave as f32 * 0.5).rem_euclid(1.0);
            let front = t * 1.15 - 0.02;
            (1.0 - t).powf(0.7) * (-((distance - front) / 0.085).powi(2)).exp()
        })
        .sum::<f32>()
        .clamp(0.0, 1.0)
}

/// The field's cells for `layout`, at a point of the build (`None` once
/// built) and of the pulse (`None` while it rests, under reduced motion).
/// The grid puts the heart on a grid crossing, so four cells share it, as
/// in the icon; cells at or past the reach are left out.
pub fn field_cells(layout: &FieldLayout, build: Option<f32>, pulse: Option<f32>) -> Vec<FieldCell> {
    let pitch = layout.pitch.max(1.0);
    let (reach_x, reach_y) = (layout.reach_x.max(pitch), layout.reach_y.max(pitch));
    let across = (reach_x / pitch).ceil() as i32 + 1;
    let up = (reach_y / pitch).ceil() as i32 + 1;
    let down = (field_below(pitch, reach_y) / pitch).ceil() as i32 + 1;
    let mut cells = Vec::new();
    for row in -up..down {
        for col in -across..across {
            let x = (col as f32 + 0.5) * pitch;
            let y = (row as f32 + 0.5) * pitch;
            // Below the heart's own row the distance counts faster, so the
            // field thins out under it and leaves the words clear.
            let weighted = if y > pitch {
                pitch + (y - pitch) * FIELD_DOWNWARD
            } else {
                y
            };
            let distance = (x / reach_x).hypot(weighted / reach_y);
            if distance >= 1.0 {
                continue;
            }
            let appear = build.map_or(1.0, |t| {
                ((t - 0.1 - distance * 0.55) / 0.35).clamp(0.0, 1.0)
            });
            cells.push(FieldCell {
                x,
                y,
                extent: pitch * CELL_SHARE * (1.0 - CELL_SHRINK * smoothstep(0.0, 1.0, distance)),
                distance,
                appear,
                lift: pulse.map_or(0.0, |phase| pulse_lift(distance, phase)),
            });
        }
    }
    cells
}

/// How far below the heart the field reaches: the heart's own row, then the
/// rest of the upward reach counted `FIELD_DOWNWARD` times faster.
fn field_below(pitch: f32, reach: f32) -> f32 {
    pitch + (reach - pitch).max(0.0) / FIELD_DOWNWARD
}

/// Which of `shades` (heart to edge) a cell at `distance` takes: the icon's
/// even split, spread over `SHADE_REACH` of the field.
pub fn shade_index(distance: f32, shades: usize) -> usize {
    let shades = shades.max(1);
    let t = (distance / SHADE_REACH).clamp(0.0, 1.0);
    ((t * (shades as f32 - 0.01)) as usize).min(shades - 1)
}

/// How visible a cell at `distance` is: whole around the heart, dissolving
/// to nothing at the reach.
pub fn field_alpha(distance: f32) -> f32 {
    1.0 - smoothstep(FADE_START, 1.0, distance)
}

/// What the field draws with: the cells' shades from the heart outwards and
/// the highlight a pulse lifts a cell towards. Brand colours, resolved by
/// the host.
#[derive(Clone, Debug)]
pub struct FieldStyle {
    pub shades: Vec<Hsla>,
    pub highlight: Hsla,
}

/// The straight mix of two colours at `t` (0 is `a`).
pub fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b, t) = (Rgba::from(a), Rgba::from(b), t.clamp(0.0, 1.0));
    Hsla::from(Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    })
}

/// A cell's colour: its shade, lifted towards the highlight while a pulse
/// passes, faded by its distance and while it appears.
pub fn cell_color(cell: &FieldCell, style: &FieldStyle) -> Hsla {
    let Some(shade) = style
        .shades
        .get(shade_index(cell.distance, style.shades.len()))
        .copied()
    else {
        return style.highlight.opacity(0.0);
    };
    let color = mix(shade, style.highlight, cell.lift * PULSE_LIGHT);
    let alpha = field_alpha(cell.distance) * (1.0 + 0.6 * cell.lift).min(1.4);
    color.opacity(alpha.min(1.0) * appear_alpha(cell.appear))
}

/// A cell's scale while it appears: out from nothing with a small
/// overshoot (an ease-out-back), settling on 1.
pub fn appear_scale(appear: f32) -> f32 {
    let t = appear.clamp(0.0, 1.0);
    let s = 1.7;
    let u = t - 1.0;
    1.0 + (s + 1.0) * u * u * u + s * u * u
}

fn appear_alpha(appear: f32) -> f32 {
    smoothstep(0.0, 0.6, appear)
}

/// The heart of the welcome: a `layout.side` square in the column, with the
/// field painted around its centre and spilling past it on every side (the
/// host's canvas clips it). Later siblings, the question and the action,
/// paint over the field.
pub fn hero(layout: &FieldLayout, cells: Vec<FieldCell>, style: &FieldStyle) -> Div {
    let style = style.clone();
    let width = 2.0 * (layout.reach_x + layout.pitch);
    let up = layout.reach_y + layout.pitch;
    let down = field_below(layout.pitch, layout.reach_y) + layout.pitch;
    div().relative().size(px(layout.side)).flex_none().child(
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let heart = bounds.origin + point(px(width * 0.5), px(up));
                for cell in &cells {
                    if cell.appear <= 0.0 {
                        continue;
                    }
                    let color = cell_color(cell, &style);
                    if color.a <= 0.003 {
                        continue;
                    }
                    let side =
                        cell.extent * appear_scale(cell.appear) * (1.0 + PULSE_GROW * cell.lift);
                    window.paint_quad(
                        fill(
                            Bounds {
                                origin: heart
                                    + point(px(cell.x - side * 0.5), px(cell.y - side * 0.5)),
                                size: size(px(side), px(side)),
                            },
                            color,
                        )
                        .corner_radii(px(side * CELL_ROUND)),
                    );
                }
            },
        )
        .absolute()
        .left(px((layout.side - width) * 0.5))
        .top(px(layout.side * 0.5 - up))
        .w(px(width))
        .h(px(up + down)),
    )
}

/// The question under the heart: `HEADING_GAP` above it, `HEADING_SIZE`
/// semibold.
pub fn heading(color: Hsla, copy: impl Into<SharedString>) -> Div {
    div()
        .mt(px(HEADING_GAP))
        .text_size(ui_rems(HEADING_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(copy.into())
}

/// The line under the question: `SUBHEADING_GAP` above it, centred and
/// wrapped at `SUBHEADING_WIDTH`.
pub fn subheading(color: Hsla, copy: impl Into<SharedString>) -> Div {
    div()
        .mt(px(SUBHEADING_GAP))
        .max_w(rems(SUBHEADING_WIDTH / 16.0))
        .text_size(ui_rems(SUBHEADING_SIZE))
        .line_height(ui_rems(SUBHEADING_LINE))
        .text_center()
        .text_color(color)
        .child(copy.into())
}

/// Sets the host's action button under the line: `ACTION_GAP` above it,
/// padded `ACTION_PAD_X` by `ACTION_PAD_Y`, its label at `ACTION_TEXT`.
pub fn action<E: Styled>(button: E) -> E {
    button
        .mt(px(ACTION_GAP))
        .px(px(ACTION_PAD_X))
        .py(px(ACTION_PAD_Y))
        .text_size(ui_rems(ACTION_TEXT))
}

/// What the shortcut hint draws with: its words, the keycap's fill and
/// text, and the mono family the keys are set in.
#[derive(Clone, Debug)]
pub struct HintStyle {
    pub text: Hsla,
    pub keycap: Hsla,
    pub keycap_text: Hsla,
    pub mono: SharedString,
}

/// The action's shortcut under it, `HINT_GAP` below: `lead` ("or press")
/// and the keys in a keycap.
pub fn hint(
    style: &HintStyle,
    lead: impl Into<SharedString>,
    keys: impl Into<SharedString>,
) -> Div {
    div()
        .mt(px(HINT_GAP))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(ui_rems(HINT_SIZE))
        .text_color(style.text)
        .child(lead.into())
        .child(
            div()
                .px(px(KEYCAP_PAD_X))
                .py(px(KEYCAP_PAD_Y))
                .rounded(px(KEYCAP_RADIUS))
                .bg(style.keycap)
                .text_color(style.keycap_text)
                .font_family(style.mono.clone())
                .child(keys.into()),
        )
}

/// The group: the heart, the question, the line, the action and its hint in
/// one centred column.
pub fn welcome() -> Div {
    div().flex().flex_col().items_center()
}
