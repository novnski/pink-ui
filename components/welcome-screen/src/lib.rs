// PINK's first-run welcome (welcome_mark.rs and the onboarding canvas in
// shell.rs): PINK's mark over one question, a line on what to do, the one
// action that starts a project and its shortcut.
//
// The mark is the app icon's idea at hero size: a rounded square tile made
// of `GRID` x `GRID` rounded cells, light pink at its heart and deepening
// shade by shade to its edges. Under the pointer a ripple runs out from the
// centre once, over `RIPPLE_SECONDS`, lifting each ring as it passes, and
// the tile settles again. The cells are painted quads, so the mark looks the
// same in every font, size and appearance.
//
// The package draws only. The host owns the clock (when a ripple started,
// reduced motion, animation frames), the resolved colours and mono font,
// the action button with its id, role, accessible name and listener, and
// the mark's role, accessible name and hover listener. The text is rems, so
// it follows the host's Interface size; the mark is sized in pixels from the
// space it has. Plain `//` comments: the pink-ui gallery compiles this file
// in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Bounds, Div, FontWeight, Hsla, Rgba, SharedString, canvas, div, fill, point,
    prelude::*, px, rems, size,
};

/// Cells across (and down) the tile. Odd, so one cell sits at the heart.
pub const GRID: usize = 9;
/// The gap between neighbouring cells, as a share of the cell pitch.
pub const CELL_GAP: f32 = 0.16;
/// A cell's corner radius, as a share of its side.
pub const CELL_ROUND: f32 = 0.26;
/// How far a passing ripple grows a cell, as a share of its side.
pub const RIPPLE_GROW: f32 = 0.1;
/// One ripple, in seconds at normal motion speed.
pub const RIPPLE_SECONDS: f32 = 1.6;
/// The mark at full scale, and the share of it a small window keeps.
pub const MARK_SIDE: f32 = 176.0;
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

/// Where a ripple stands from its `progress` (0..1 of `RIPPLE_SECONDS`):
/// eased out, so it leaves the centre quickly and slows towards the edge;
/// `None` once it has finished (and for no ripple at all), when the host
/// stops asking for frames.
pub fn ripple_at(progress: Option<f32>) -> Option<f32> {
    let p = progress.filter(|p| *p < 1.0)?.max(0.0);
    Some(1.0 - (1.0 - p) * (1.0 - p))
}

/// One cell of the tile: its `shade` from the light heart (0) to the deep
/// edge (1), and how much a passing ripple `lift`s it (0 at rest).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub shade: f32,
    pub lift: f32,
}

/// The tile's cells, row by row (`GRID` x `GRID`), with a ripple at
/// `ripple` (from [`ripple_at`]; `None` at rest). The four corner cells are
/// `None`, which rounds the tile like the app icon.
pub fn mark_cells(ripple: Option<f32>) -> Vec<Option<Cell>> {
    let half = (GRID - 1) as f32 * 0.5;
    (0..GRID * GRID)
        .map(|index| {
            // Cell centres from -1 to 1 across the tile.
            let x = ((index % GRID) as f32 - half) / half;
            let y = ((index / GRID) as f32 - half) / half;
            // A squircle silhouette: only the outermost corners fall out.
            if x.powi(4) + y.powi(4) > 1.5 {
                return None;
            }
            // Square rings softened by the round distance, so the shades
            // step outwards like the icon's frames without hard diagonals.
            let ring = x.abs().max(y.abs());
            let round = (x * x + y * y).sqrt() / std::f32::consts::SQRT_2;
            let distance = 0.65 * ring + 0.35 * round;
            let lift = ripple.map_or(0.0, |t| {
                let front = t * 1.35;
                (1.0 - t) * (-((distance - front) / 0.22).powi(2)).exp()
            });
            Some(Cell {
                shade: distance.clamp(0.0, 1.0),
                lift,
            })
        })
        .collect()
}

/// What the mark draws with: the shade at its heart and at its edge, and
/// the highlight a ripple lifts a cell towards. Brand colours, resolved by
/// the host.
#[derive(Clone, Debug)]
pub struct MarkStyle {
    pub heart: Hsla,
    pub edge: Hsla,
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

/// A cell's colour: its shade between the heart and the edge, lifted
/// towards the highlight while a ripple passes.
pub fn cell_color(cell: Cell, style: &MarkStyle) -> Hsla {
    mix(
        mix(style.heart, style.edge, cell.shade),
        style.highlight,
        cell.lift * 0.6,
    )
}

/// The mark: `cells` (from [`mark_cells`]) painted as rounded quads in a
/// `side` square.
pub fn mark(cells: Vec<Option<Cell>>, side: f32, style: &MarkStyle) -> Div {
    let style = style.clone();
    let pitch = side / GRID as f32;
    div().size(px(side)).flex_none().child(
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                for (index, cell) in cells.iter().enumerate() {
                    let Some(cell) = *cell else {
                        continue;
                    };
                    let extent = pitch * (1.0 - CELL_GAP) * (1.0 + RIPPLE_GROW * cell.lift);
                    let x = (index % GRID) as f32 * pitch + (pitch - extent) * 0.5;
                    let y = (index / GRID) as f32 * pitch + (pitch - extent) * 0.5;
                    window.paint_quad(
                        fill(
                            Bounds {
                                origin: bounds.origin + point(px(x), px(y)),
                                size: size(px(extent), px(extent)),
                            },
                            cell_color(cell, &style),
                        )
                        .corner_radii(px(extent * CELL_ROUND)),
                    );
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
