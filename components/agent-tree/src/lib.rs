// PINK's agent tree (shell/agents.rs `render_agents_section`): the Agents
// list nested, each sub-agent under the agent that started it. The rows are
// `pink-agent-row`; this package draws the outline around them: each
// depth's indent, the activity rail (a 1 px hairline trunk off the parent's
// dot, a 6 pt elbow and the branch to the child's dot, the same ribbon as
// the transcript's `activity_rail`) and the disclosure tile with its
// chevron.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours, the
// chevron's icon path, the fold progress and how much of each rail is drawn;
// it adds the ids, tree roles and accessible names, focus, keyboard, hover
// and selection fills and the actions. Plain `//` comments: the pink-ui
// gallery compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, Hsla, PathBuilder, Pixels, Point, SharedString, Transformation, Window,
    canvas, div, point, prelude::*, px, radians, rems, svg,
};

// The row the outline nests, in pt at the 16 px default
// (`pink-agent-row`: PAD_X, PAD_Y, MARK, LIST_GAP and its line heights).
pub const ROW_PAD_X: f32 = 10.0;
pub const ROW_PAD_Y: f32 = 7.0;
pub const MARK: f32 = 7.0;
pub const LINE_HEIGHT: f32 = 1.618_034;
pub const TITLE_LINE: f32 = 13.0 * LINE_HEIGHT;
pub const ROW_HEIGHT: f32 = 2.0 * ROW_PAD_Y + TITLE_LINE + 2.0 + 11.5 * LINE_HEIGHT;
// Between one row and the next; a slot is a row and the gap under it.
pub const ROW_GAP: f32 = 2.0;
pub const SLOT_H: f32 = ROW_HEIGHT + ROW_GAP;
// The status dot's centre, from the slot's top.
pub const DOT_CY: f32 = ROW_PAD_Y + TITLE_LINE / 2.0;

// The activity rail (the transcript's `activity_rail`: trunk at x 12.5,
// elbow radius 6, branch to x 28, glyph at x 32), at `RAIL_ALPHA` of the
// hairline colour.
pub const RAIL_ALPHA: f32 = 0.12;
pub const BEND_RADIUS: f32 = 6.0;
// From a trunk to the child's dot (its leading edge).
pub const RAIL_SPAN: f32 = 32.0 - 12.5;
// The gap the branch leaves before the dot.
pub const BRANCH_GAP: f32 = 32.0 - 28.0;
// A depth-0 row's trunk runs through its dot's centre.
pub const TRUNK_X: f32 = ROW_PAD_X + MARK / 2.0;
// One depth: a child's dot starts `RAIL_SPAN` right of its parent's trunk.
pub const INDENT: f32 = TRUNK_X + RAIL_SPAN - ROW_PAD_X;
// A parent's trunk starts just under its dot.
pub const RAIL_TOP: f32 = DOT_CY + MARK / 2.0 + BRANCH_GAP;

// The disclosure tile (the transcript's `chip_header_row` trail) and its
// chevron, which turns 0 to 90 degrees as the branch opens.
pub const TILE: f32 = 18.0;
pub const CHEVRON: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// A row's leading padding at `depth` (0 is a root), in pt at the 16 px
/// default: its dot sits `INDENT` right of its parent's, starting
/// `RAIL_SPAN` right of the parent's trunk.
pub fn indent(depth: usize) -> f32 {
    ROW_PAD_X + depth as f32 * INDENT
}

/// The x of the trunk a row at `level` starts for its sub-agents.
pub fn trunk_x(level: usize) -> f32 {
    level as f32 * INDENT + TRUNK_X
}

/// What one slot's rail draws, in pt from the slot's top-left corner.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Rail {
    /// Ancestor trunks passing this slot: x and opacity.
    pub guides: Vec<(f32, f32)>,
    /// The elbow off the parent's trunk.
    pub elbow: Option<Elbow>,
    /// The trunk this row starts for its sub-agents: x, drawn share,
    /// opacity.
    pub start: Option<(f32, f32, f32)>,
}

/// The parent's trunk into this row and the branch to its dot, each as a
/// drawn share from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elbow {
    pub x: f32,
    /// Trunk from the slot top to the bend.
    pub incoming: f32,
    /// The bend and the branch to the dot.
    pub branch: f32,
    /// Trunk from the bend on to the slot bottom, for a later sibling.
    pub continuation: f32,
}

impl Rail {
    /// A settled rail (nothing arriving or folding): the slot of a row at
    /// `depth`, where `ancestors_continue[level]` says whether the ancestor
    /// at that level has a later sibling (its trunk passes this slot) and
    /// the last entry says whether this row does. `open` starts the row's
    /// own trunk for its visible sub-agents.
    pub fn settled(depth: usize, ancestors_continue: &[bool], open: bool) -> Self {
        let mut rail = Rail::default();
        if depth > 0 {
            for (level, continues) in ancestors_continue.iter().enumerate().take(depth - 1) {
                if *continues {
                    rail.guides.push((trunk_x(level), 1.0));
                }
            }
            let later = ancestors_continue.get(depth - 1).copied().unwrap_or(false);
            rail.elbow = Some(Elbow {
                x: trunk_x(depth - 1),
                incoming: 1.0,
                branch: 1.0,
                continuation: if later { 1.0 } else { 0.0 },
            });
        }
        if open {
            rail.start = Some((trunk_x(depth), 1.0, 1.0));
        }
        rail
    }
}

/// The rail over a slot, absolutely placed at its top-left corner. `scale`
/// is the window's rem size over 16 (the host's Interface size).
pub fn rail(rail: Rail, color: Hsla, scale: f32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| paint_rail(&rail, bounds.origin, color, scale, window),
    )
    .absolute()
    .top_0()
    .left_0()
    .w_full()
    .h(px(SLOT_H * scale))
}

fn union_path() -> PathBuilder {
    PathBuilder::fill().with_style(gpui::PathStyle::Fill(
        gpui::FillOptions::default().with_fill_rule(gpui::FillRule::NonZero),
    ))
}

/// Paints a slot's rail. Fully drawn segments share one nonzero path so the
/// joints never double their alpha; fading ones paint on their own.
pub fn paint_rail(
    rail: &Rail,
    origin: Point<Pixels>,
    color: Hsla,
    scale: f32,
    window: &mut Window,
) {
    let at = |x: f32, y: f32| point(origin.x + px(x * scale), origin.y + px(y * scale));
    let mut solid = union_path();
    let mut faded: Vec<(PathBuilder, f32)> = Vec::new();
    let mut add = |points: &[Point<Pixels>], alpha: f32| {
        if alpha >= 1. {
            ribbon(&mut solid, points);
        } else if alpha > 0. {
            let mut path = union_path();
            ribbon(&mut path, points);
            faded.push((path, alpha));
        }
    };
    for &(x, alpha) in &rail.guides {
        add(&[at(x, 0.), at(x, SLOT_H)], alpha);
    }
    if let Some(elbow) = rail.elbow {
        let bend_y = DOT_CY - BEND_RADIUS;
        if elbow.incoming > 0. {
            let mut bottom = bend_y * elbow.incoming;
            if elbow.incoming >= 1. && elbow.continuation > 0. {
                bottom = bend_y + (SLOT_H - bend_y) * elbow.continuation;
            }
            add(&[at(elbow.x, 0.), at(elbow.x, bottom)], 1.);
        }
        if elbow.branch > 0. {
            let points: Vec<_> = branch_points(elbow.branch)
                .into_iter()
                .map(|p| at(elbow.x + p.x, bend_y + p.y))
                .collect();
            add(&points, 1.);
        }
    }
    if let Some((x, drawn, alpha)) = rail.start {
        let bottom = RAIL_TOP + (SLOT_H - RAIL_TOP) * drawn;
        add(&[at(x, RAIL_TOP), at(x, bottom)], alpha);
    }
    if let Ok(path) = solid.build() {
        window.paint_path(path, color);
    }
    for (path, alpha) in faded {
        if let Ok(path) = path.build() {
            window.paint_path(path, color.opacity(alpha));
        }
    }
}

/// The elbow and branch from the bend's top, revealed by distance so the
/// bend and the straight leg draw at one speed (the transcript's
/// `activity_branch_points`).
pub fn branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<_> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0;
            point(BEND_RADIUS * t * t, BEND_RADIUS * (2.0 * t - t * t))
        })
        .collect();
    path.push(point(RAIL_SPAN - BRANCH_GAP, BEND_RADIUS));
    if progress >= 1.0 {
        return path;
    }
    let lengths: Vec<_> = path
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .collect();
    let mut remaining = lengths.iter().sum::<f32>() * progress.clamp(0.0, 1.0);
    let mut visible = vec![path[0]];
    for (pair, length) in path.windows(2).zip(lengths) {
        if remaining <= 0.0 {
            break;
        }
        let t = (remaining / length).min(1.0);
        visible.push(point(
            pair[0].x + (pair[1].x - pair[0].x) * t,
            pair[0].y + (pair[1].y - pair[0].y) * t,
        ));
        remaining -= length;
    }
    visible
}

/// A 1 px ribbon along `points`, as a clockwise contour (the transcript's
/// `activity_ribbon`).
fn ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
    if points.len() < 2 {
        return;
    }
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for (ix, p) in points.iter().enumerate() {
        let a = points[ix.saturating_sub(1)];
        let b = points[(ix + 1).min(points.len() - 1)];
        let dx = f32::from(b.x - a.x);
        let dy = f32::from(b.y - a.y);
        let length = dx.hypot(dy).max(0.0001);
        let normal = point(px(-dy / length * 0.5), px(dx / length * 0.5));
        left.push(*p + normal);
        right.push(*p - normal);
    }
    path.move_to(left[0]);
    for p in left.iter().skip(1).chain(right.iter().rev()) {
        path.line_to(*p);
    }
    path.close();
}

/// The disclosure tile: the chevron at `icon`, turned 90 degrees times
/// `open` (0 folded, 1 open), in `color`, or the second colour while the
/// pointer is over the named row group. The host adds the id, the tile's
/// hover strength and the toggle.
pub fn disclosure(
    icon: impl Into<SharedString>,
    open: f32,
    color: Hsla,
    group_hover: Option<(&'static str, Hsla)>,
) -> Div {
    div()
        .size(ui_rems(TILE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(icon.into())
                .size(ui_rems(CHEVRON))
                .with_transformation(Transformation::rotate(radians(
                    std::f32::consts::FRAC_PI_2 * open.clamp(0.0, 1.0),
                )))
                .text_color(color)
                .when_some(group_hover, |icon, (group, hover)| {
                    icon.group_hover(group, move |s| s.text_color(hover))
                }),
        )
}

/// A leaf's empty tile slot, so every status word ends on one column.
pub fn tile_slot() -> Div {
    div().size(ui_rems(TILE)).flex_none()
}
