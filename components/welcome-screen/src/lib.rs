// PINK's first-run welcome (welcome_mark.rs and the onboarding canvas in
// shell.rs): PINK's mark over one question, a line on what to do, the one
// action that starts a project and its shortcut.
//
// The mark is the app icon drawn live: a deep pink rounded tile holding a
// `GRID` x `GRID` field of rounded cells, each shaded by its distance from
// the centre, lightest at the heart and deepest at the edges. Its geometry
// is the icon's (an 824 pt tile with 16 pt gaps), so the two read as one
// object. When it first appears the tile settles in and the cells pop in
// from the centre outwards over `BUILD_SECONDS`; under the pointer a ripple
// runs out from the centre once, over `RIPPLE_SECONDS`, lifting each ring
// towards the highlight as it passes. The cells are painted quads (the four
// corner cells are clipped to the tile's rounded corners, as the icon's
// mask clips them), so the mark looks the same in every font and size.
//
// The package draws only. The host owns the clocks (when the build and a
// ripple started, reduced motion, animation frames), the resolved colours
// and mono font, the action button with its id, role, accessible name and
// listener, and the mark's role, accessible name and hover listener. The
// text is rems, so it follows the host's Interface size; the mark is sized
// in pixels from the space it has. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its story.

use std::f32::consts::{PI, TAU};

use gpui::{
    AbsoluteLength, Bounds, Div, FontWeight, Hsla, Path, Pixels, Point, Rgba, SharedString,
    canvas, div, fill, point, prelude::*, px, rems, size,
};

/// Cells across (and down) the tile, as in the app icon.
pub const GRID: usize = 6;
/// The icon's geometry as shares of the tile's side: the gap around every
/// cell (16 pt of 824) and the tile's corner radius (185.4 pt of 824).
pub const CELL_GAP: f32 = 16.0 / 824.0;
pub const TILE_ROUND: f32 = 185.4 / 824.0;
/// A cell's corner radius, as a share of its side.
pub const CELL_ROUND: f32 = 0.22;
/// How far a passing ripple grows a cell, as a share of its side.
pub const RIPPLE_GROW: f32 = 0.08;
/// How strongly a passing ripple moves a cell towards the highlight.
pub const RIPPLE_LIGHT: f32 = 0.55;
/// The build when the mark first appears, and one ripple, in seconds at
/// normal motion speed.
pub const BUILD_SECONDS: f32 = 0.7;
pub const RIPPLE_SECONDS: f32 = 1.4;
/// The mark at full scale, and the share of it a small window keeps.
pub const MARK_SIDE: f32 = 168.0;
pub const MARK_MIN_SCALE: f32 = 0.6;
/// The question under the mark: the space above it, its size.
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

/// The mark's side for a column `width` wide in a window `height` tall:
/// full size from 900 tall and 440 wide, shrinking with the smaller of the
/// two down to `MARK_MIN_SCALE`.
pub fn mark_side(width: f32, height: f32) -> f32 {
    MARK_SIDE * (height / 900.0).min(width / 440.0).clamp(MARK_MIN_SCALE, 1.0)
}

/// Where a build stands from its `progress` (0..1 of `BUILD_SECONDS`), or
/// `None` once it has finished (and for no build at all).
pub fn build_at(progress: Option<f32>) -> Option<f32> {
    progress.filter(|p| *p < 1.0).map(|p| p.max(0.0))
}

/// Where a ripple stands from its `progress` (0..1 of `RIPPLE_SECONDS`):
/// eased out, so it leaves the centre quickly and slows towards the edge;
/// `None` once it has finished (and for no ripple at all).
pub fn ripple_at(progress: Option<f32>) -> Option<f32> {
    let p = progress.filter(|p| *p < 1.0)?.max(0.0);
    Some(1.0 - (1.0 - p) * (1.0 - p))
}

/// One cell of the tile: its `distance` from the centre (from the inner
/// ring to 1 at the corners, which picks its shade), how far it has
/// `appear`ed in the build (0 to 1; 1 at rest) and how much a passing
/// ripple `lift`s it (0 at rest).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub distance: f32,
    pub appear: f32,
    pub lift: f32,
}

/// One frame of the mark: the tile's own build (0 to 1) and its cells, row
/// by row (`GRID` x `GRID`).
#[derive(Clone, Debug, PartialEq)]
pub struct MarkFrame {
    pub tile: f32,
    pub cells: Vec<Cell>,
}

/// The mark at a point of its build (`None` once built) and of a ripple
/// (`None` at rest). In the build the tile settles over its first half and
/// each cell pops in after a delay that grows with its distance from the
/// centre, so the mark fills from its heart outwards.
pub fn mark_frame(build: Option<f32>, ripple: Option<f32>) -> MarkFrame {
    let half = (GRID - 1) as f32 * 0.5;
    let reach = (2.0 * half * half).sqrt();
    let cells = (0..GRID * GRID)
        .map(|index| {
            let dx = (index % GRID) as f32 - half;
            let dy = (index / GRID) as f32 - half;
            let distance = (dx * dx + dy * dy).sqrt() / reach;
            let appear = build.map_or(1.0, |t| ((t - 0.15 - distance * 0.4) / 0.45).clamp(0.0, 1.0));
            let lift = ripple.map_or(0.0, |t| {
                let front = t * 1.5;
                (1.0 - t) * (-((distance - front) / 0.2).powi(2)).exp()
            });
            Cell {
                distance,
                appear,
                lift,
            }
        })
        .collect();
    MarkFrame {
        tile: build.map_or(1.0, |t| (t / 0.5).clamp(0.0, 1.0)),
        cells,
    }
}

/// Which of `shades` (heart to edge) a cell at `distance` takes: the icon's
/// even split of the distance range.
pub fn shade_index(distance: f32, shades: usize) -> usize {
    let shades = shades.max(1);
    ((distance.clamp(0.0, 1.0) * (shades as f32 - 0.01)) as usize).min(shades - 1)
}

/// What the mark draws with: the tile under the cells, the cells' shades
/// from the heart to the edge, and the highlight a ripple lifts a cell
/// towards. Brand colours, resolved by the host.
#[derive(Clone, Debug)]
pub struct MarkStyle {
    pub tile: Hsla,
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

/// A cell's colour: its shade, lifted towards the highlight while a ripple
/// passes, and faded in while it appears.
pub fn cell_color(cell: Cell, style: &MarkStyle) -> Hsla {
    let shade = style
        .shades
        .get(shade_index(cell.distance, style.shades.len()))
        .copied()
        .unwrap_or(style.tile);
    let color = mix(shade, style.highlight, cell.lift * RIPPLE_LIGHT);
    color.opacity(appear_alpha(cell.appear))
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
    let t = (appear * 1.6).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The cell rectangle at grid position `col`, `row` in a `side` square, as
/// (left, top, extent) in the square's own pixels, before any scaling.
pub fn cell_rect(side: f32, col: usize, row: usize) -> (f32, f32, f32) {
    let gap = CELL_GAP * side;
    let extent = (side - gap * (GRID as f32 + 1.0)) / GRID as f32;
    (
        gap + col as f32 * (extent + gap),
        gap + row as f32 * (extent + gap),
        extent,
    )
}

/// The outline of a rounded square at `(x, y)` with side `extent` and
/// corner `radius`, clockwise from its top edge, with `steps` points per
/// corner.
fn rounded_square(x: f32, y: f32, extent: f32, radius: f32, steps: usize) -> Vec<(f32, f32)> {
    let r = radius.min(extent * 0.5);
    let corners = [
        (x + extent - r, y + r, -PI * 0.5),
        (x + extent - r, y + extent - r, 0.0),
        (x + r, y + extent - r, PI * 0.5),
        (x + r, y + r, PI),
    ];
    let mut outline = Vec::with_capacity(corners.len() * (steps + 1));
    for (cx, cy, start) in corners {
        for step in 0..=steps {
            let angle = start + PI * 0.5 * step as f32 / steps as f32;
            outline.push((cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    outline
}

/// A convex outline clipped to the disc at `center` of `radius`: the parts
/// inside are kept and the cut is closed along the circle. Only the tile's
/// corner cells need it; their corner region of the tile is that disc.
pub fn clip_to_disc(outline: &[(f32, f32)], center: (f32, f32), radius: f32) -> Vec<(f32, f32)> {
    let inside = |p: (f32, f32)| (p.0 - center.0).hypot(p.1 - center.1) <= radius;
    let angle = |p: (f32, f32)| (p.1 - center.1).atan2(p.0 - center.0);
    let on_circle = |a: f32| (center.0 + radius * a.cos(), center.1 + radius * a.sin());
    // Where segment `a`..`b` crosses the circle, as fractions along it.
    let crossings = |a: (f32, f32), b: (f32, f32)| -> Vec<f32> {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let (fx, fy) = (a.0 - center.0, a.1 - center.1);
        let qa = dx * dx + dy * dy;
        let qb = 2.0 * (fx * dx + fy * dy);
        let qc = fx * fx + fy * fy - radius * radius;
        let disc = qb * qb - 4.0 * qa * qc;
        if qa <= f32::EPSILON || disc < 0.0 {
            return Vec::new();
        }
        let root = disc.sqrt();
        let mut ts: Vec<f32> = [(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
            .into_iter()
            .filter(|t| (0.0..=1.0).contains(t))
            .collect();
        ts.sort_by(|a, b| a.total_cmp(b));
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        ts
    };
    let mut clipped = Vec::new();
    let mut exit: Option<f32> = None;
    let mut first_entry: Option<f32> = None;
    let mut enter = |clipped: &mut Vec<(f32, f32)>, exit: &mut Option<f32>, at: f32| {
        match exit.take() {
            // Close the cut along the circle, the short way round.
            Some(from) => {
                let mut sweep = (at - from).rem_euclid(TAU);
                if sweep > PI {
                    sweep -= TAU;
                }
                let steps = ((sweep.abs() / (PI / 24.0)).ceil() as usize).max(1);
                for step in 1..steps {
                    clipped.push(on_circle(from + sweep * step as f32 / steps as f32));
                }
            }
            None => first_entry = first_entry.or(Some(at)),
        }
        clipped.push(on_circle(at));
    };
    for (index, &a) in outline.iter().enumerate() {
        let b = outline[(index + 1) % outline.len()];
        if inside(a) {
            clipped.push(a);
        }
        for t in crossings(a, b) {
            let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let leaving = inside((a.0 + (b.0 - a.0) * (t - 1e-3), a.1 + (b.1 - a.1) * (t - 1e-3)));
            if leaving {
                clipped.push(p);
                exit = Some(angle(p));
            } else {
                enter(&mut clipped, &mut exit, angle(p));
            }
        }
    }
    // A cut that wraps past the outline's start closes onto its first entry.
    if let (Some(from), Some(to)) = (exit, first_entry) {
        let mut sweep = (to - from).rem_euclid(TAU);
        if sweep > PI {
            sweep -= TAU;
        }
        let steps = ((sweep.abs() / (PI / 24.0)).ceil() as usize).max(1);
        for step in 1..steps {
            clipped.push(on_circle(from + sweep * step as f32 / steps as f32));
        }
    }
    clipped
}

/// The mark: `frame` (from [`mark_frame`]) painted in a `side` square, the
/// tile first and every cell over it.
pub fn mark(frame: MarkFrame, side: f32, style: &MarkStyle) -> Div {
    let style = style.clone();
    div().size(px(side)).flex_none().child(
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let at = |x: f32, y: f32| -> Point<Pixels> { bounds.origin + point(px(x), px(y)) };
                // The tile settles in from 94% as it fades up.
                let tile_scale = 0.94 + 0.06 * frame.tile;
                let tile_side = side * tile_scale;
                let inset = (side - tile_side) * 0.5;
                window.paint_quad(
                    fill(
                        Bounds {
                            origin: at(inset, inset),
                            size: size(px(tile_side), px(tile_side)),
                        },
                        style.tile.opacity(appear_alpha(frame.tile)),
                    )
                    .corner_radii(px(TILE_ROUND * tile_side)),
                );
                let round = TILE_ROUND * side;
                let last = GRID - 1;
                for (index, cell) in frame.cells.iter().enumerate() {
                    if cell.appear <= 0.0 {
                        continue;
                    }
                    let (col, row) = (index % GRID, index / GRID);
                    let (x, y, extent) = cell_rect(side, col, row);
                    let grown =
                        extent * appear_scale(cell.appear) * (1.0 + RIPPLE_GROW * cell.lift);
                    let (x, y) = (x + (extent - grown) * 0.5, y + (extent - grown) * 0.5);
                    let color = cell_color(*cell, &style);
                    let corner = (col == 0 || col == last) && (row == 0 || row == last);
                    if !corner {
                        window.paint_quad(
                            fill(
                                Bounds {
                                    origin: at(x, y),
                                    size: size(px(grown), px(grown)),
                                },
                                color,
                            )
                            .corner_radii(px(grown * CELL_ROUND)),
                        );
                        continue;
                    }
                    // A corner cell, clipped to the tile's rounded corner
                    // as the icon's mask clips it.
                    let center = (
                        if col == 0 { round } else { side - round },
                        if row == 0 { round } else { side - round },
                    );
                    let outline = clip_to_disc(
                        &rounded_square(x, y, grown, grown * CELL_ROUND, 6),
                        center,
                        round,
                    );
                    if let Some((&first, rest)) = outline.split_first() {
                        let mut path = Path::new(at(first.0, first.1));
                        for &(px_, py_) in rest {
                            path.line_to(at(px_, py_));
                        }
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size_full(),
    )
}

/// The question under the mark: `HEADING_GAP` above it, `HEADING_SIZE`
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

/// The group: the mark, the question, the line, the action and its hint in
/// one centred column.
pub fn welcome() -> Div {
    div().flex().flex_col().items_center()
}
