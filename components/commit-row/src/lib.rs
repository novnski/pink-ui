// PINK's commit history rows (history.rs `render_row`, `render_ref`,
// `render_ref_area`, `render_author_cell`, `render_date_cell`,
// `render_sha_cell` and the Load more row): one commit as a `ROW_HEIGHT` row
// after its graph cell, with its subject and ref badges (branch,
// remote, tag; an overflow count past the room), then the optional author
// (avatar or name), date and short-sha cells; and the button that loads
// older commits; the column header over them (names, resize dividers, the
// drop line and the columns button) and each row's graph cell (the node,
// HEAD's ring and a branch tip's fold control), and the lane graph's strokes
// painted across the rows (`paint_lanes`, `paint_rail`). UI rethink decision
// 54: drawn here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the lane graph's layout (each
// row's bounds this frame, its segments' lane positions and colour ids, the
// node radius under focus, the focused lane and its fade), the glyphs and
// avatar images (its own assets), every colour already resolved (the row's
// and the sha's washes blended over their hover fades, the graph focus's
// dimming as an opacity), the families and the text; it adds the ids'
// listeners, tooltips, the container query that sizes the ref area, column
// widths and their persistence, the fold transition, copying a sha and
// opening a commit. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Bounds, Div, ElementId, Hsla, Img, ObjectFit, PathBuilder, Pixels, Point,
    SharedString, Stateful, StyledImage, Svg, Window, div, point, prelude::*, px, rems,
};

/// A commit row's height, in fixed pixels (PINK `HISTORY_ROW_HEIGHT`).
pub const ROW_HEIGHT: f32 = 36.0;
/// The row's type and the subject's, in pt at the 16 px default.
pub const ROW_TEXT: f32 = 11.0;
pub const SUBJECT_TEXT: f32 = 12.0;
/// The subject column's narrowest, in fixed pixels (PINK
/// `HISTORY_COMMIT_SUBJECT_MIN_WIDTH`).
pub const SUBJECT_MIN_WIDTH: f32 = 80.0;
/// The gap between the subject and the ref badges, and between badges, in
/// fixed pixels (PINK `HISTORY_REF_GAP`).
pub const REF_GAP: f32 = 5.0;
/// The trailing inset after the subject and refs, and after the author and
/// date cells, in fixed pixels.
pub const CELL_PAD_RIGHT: f32 = 8.0;
/// A ref badge's height, widest, side pad and corner, in fixed pixels (PINK
/// `HISTORY_REF_BADGE_MAX_WIDTH`).
pub const REF_BADGE_HEIGHT: f32 = 16.0;
pub const REF_BADGE_MAX_WIDTH: f32 = 112.0;
pub const REF_BADGE_PAD_X: f32 = 5.0;
pub const REF_BADGE_RADIUS: f32 = 4.0;
/// A ref badge's glyph, in fixed pixels.
pub const REF_GLYPH: f32 = 10.0;
/// The author avatar's side, in fixed pixels.
pub const AVATAR: f32 = 20.0;
/// The sha pill's height and the sha cell's trailing inset, in fixed pixels.
pub const SHA_PILL_HEIGHT: f32 = 24.0;
pub const SHA_PAD_RIGHT: f32 = 6.0;
/// The Load more row's height and its button's, in fixed pixels.
pub const LOAD_MORE_ROW: f32 = 48.0;
pub const LOAD_MORE_HEIGHT: f32 = 28.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

// A flex factor through the style, so the file compiles on crates.io GPUI
// 0.2.2 (whose `flex_shrink` takes no factor).
fn shrink<E: Styled>(mut el: E, factor: f32) -> E {
    el.style().flex_shrink = Some(factor);
    el
}

/// One commit row: `ROW_HEIGHT` tall and full width on `wash` (already
/// blended over the hover fade, or the focused lane's wash). The host adds
/// the graph cell, the subject (`subject_lane`), the cells (`cells`) and the
/// listeners.
pub fn row(id: impl Into<ElementId>, wash: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .text_size(ui_rems(ROW_TEXT))
        .cursor_pointer()
        .bg(wash)
}

/// The subject and its refs, `REF_GAP` apart, dimmed to `opacity` while
/// another lane holds the focus. The host sizes it (at least
/// `SUBJECT_MIN_WIDTH`, taking the free width) and measures the ref area.
pub fn subject_lane(opacity: f32) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .gap(px(REF_GAP))
        .pr(px(CELL_PAD_RIGHT))
        .opacity(opacity)
}

/// The subject, taking the lane's free width and truncating.
pub fn subject(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(SUBJECT_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The ref badges' room: at most `available` wide, badges `REF_GAP` apart.
pub fn ref_area(available: f32) -> Div {
    div()
        .max_w(px(available))
        .min_w_0()
        .overflow_hidden()
        .flex()
        .items_center()
        .gap(px(REF_GAP))
}

/// What a ref badge draws, resolved by the host from the ref's colour.
pub struct RefBadgeStyle {
    /// The plate (PINK: the colour at 0.07).
    pub fill: Hsla,
    /// The name (PINK: the colour floored readable at 0.9).
    pub text: Hsla,
    /// The glyph (PINK: the colour floored readable at 0.78).
    pub glyph: Hsla,
}

/// A ref badge: the kind's glyph and the name, truncating past
/// `REF_BADGE_MAX_WIDTH`.
pub fn ref_badge(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    glyph: Svg,
    style: &RefBadgeStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(REF_BADGE_HEIGHT))
        .max_w(px(REF_BADGE_MAX_WIDTH))
        .px(px(REF_BADGE_PAD_X))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(2.0))
        .rounded(px(REF_BADGE_RADIUS))
        .bg(style.fill)
        .text_size(ui_rems(ROW_TEXT))
        .text_color(style.text)
        .child(
            glyph
                .size(px(REF_GLYPH))
                .mt(px(1.0))
                .text_color(style.glyph),
        )
        .child(div().min_w_0().truncate().child(label.into()))
}

/// The count of refs past the room (`+2`).
pub fn ref_overflow(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    color: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .text_size(ui_rems(ROW_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The optional cells after the subject, which may shrink.
pub fn cells() -> Div {
    shrink(div().h_full().flex().flex_row(), 1.0)
}

/// A column cell `width` wide (never under `min_width`), dimmed to
/// `opacity`. The cells below build on it.
pub fn cell(width: f32, min_width: f32, opacity: f32) -> Div {
    shrink(div().w(px(width)).min_w(px(min_width)).h_full(), 1.0)
        .flex()
        .items_center()
        .opacity(opacity)
}

/// The author column showing avatars: the host's `avatar` centred.
pub fn author_avatar_cell(width: f32, min_width: f32, opacity: f32) -> Div {
    cell(width, min_width, opacity).justify_center()
}

/// The author column showing names: the name truncating.
pub fn author_name_cell(
    width: f32,
    min_width: f32,
    opacity: f32,
    name: impl Into<SharedString>,
    color: Hsla,
) -> Div {
    cell(width, min_width, opacity)
        .pr(px(CELL_PAD_RIGHT))
        .truncate()
        .text_color(color)
        .child(name.into())
}

/// What the avatar draws, resolved by the host.
pub struct AvatarStyle {
    /// The ring (PINK `hairline(0.12)`).
    pub edge: Hsla,
    /// Behind the image or initial (PINK `wash(0.08)`).
    pub fill: Hsla,
}

/// The author's `AVATAR` circle; the host adds `avatar_image` or
/// `avatar_initial` and the tooltip.
pub fn avatar(id: impl Into<ElementId>, style: &AvatarStyle) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(AVATAR))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded_full()
        .border_1()
        .border_color(style.edge)
        .bg(style.fill)
}

/// The author's picture, filling the circle.
pub fn avatar_image(image: Img) -> Img {
    image
        .size_full()
        .rounded_full()
        .object_fit(ObjectFit::Cover)
}

/// The author's initial when there is no picture.
pub fn avatar_initial(text: impl Into<SharedString>, sans: SharedString, color: Hsla) -> Div {
    div()
        .w_full()
        .text_center()
        .font_family(sans)
        .text_size(ui_rems(ROW_TEXT))
        .line_height(ui_rems(18.0))
        .relative()
        .top(px(0.5))
        .text_color(color)
        .child(text.into())
}

/// The date column.
pub fn date_cell(
    width: f32,
    min_width: f32,
    opacity: f32,
    text: impl Into<SharedString>,
    color: Hsla,
) -> Div {
    cell(width, min_width, opacity)
        .truncate()
        .pr(px(CELL_PAD_RIGHT))
        .text_size(ui_rems(ROW_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The short-sha column; the host adds the `sha_pill`.
pub fn sha_cell(width: f32, min_width: f32, opacity: f32) -> Div {
    cell(width, min_width, opacity).pr(px(SHA_PAD_RIGHT))
}

/// The short sha (or "Copied") on a pill that copies it, `wash` already
/// blended over the hover fade.
pub fn sha_pill(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    mono: SharedString,
    wash: Hsla,
    color: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .h(px(SHA_PILL_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(4.0))
        .cursor_pointer()
        .bg(wash)
        .font_family(mono)
        .text_size(ui_rems(ROW_TEXT))
        .text_color(color)
        .child(label.into())
}

/// The row under the last commit that holds Load more.
pub fn load_more_row() -> Div {
    div()
        .w_full()
        .h(px(LOAD_MORE_ROW))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
}

/// What Load more draws, resolved by the host and blended over its hover
/// fade.
pub struct LoadMoreStyle {
    pub fill: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    /// The glyph's colour (PINK `text_faint`).
    pub glyph: Hsla,
    /// While a page loads the button rests with no pointer and no glyph.
    pub enabled: bool,
}

/// Load more (or Retry, or Loading...): the glyph, when enabled, then the
/// label on an outlined `LOAD_MORE_HEIGHT` plate.
pub fn load_more(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    glyph: Svg,
    style: &LoadMoreStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(LOAD_MORE_HEIGHT))
        .px(px(11.0))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .rounded(px(7.0))
        .border_1()
        .text_size(ui_rems(ROW_TEXT))
        .bg(style.fill)
        .border_color(style.border)
        .text_color(style.text)
        .when(style.enabled, |el| {
            el.cursor_pointer()
                .child(glyph.size(px(11.0)).flex_none().text_color(style.glyph))
        })
        .child(label.into())
}

// The column header (PINK history.rs `render_column_header_cell`,
// `column_resize_handle` and the header strip in `GitHistory::render`): a
// strip over the rows naming each column, a divider on each optional
// column's leading edge that resizes it, a line where a dragged header will
// land and the checklist button that shows while the strip is pointed at.

/// The header strip's height, in fixed pixels.
pub const HEADER_HEIGHT: f32 = 24.0;
/// The divider's hit width, in fixed pixels, centred on the column's edge.
pub const RESIZE_HANDLE: f32 = 6.0;
/// The drop line's width and its inset from the strip's edges, in fixed
/// pixels.
pub const DROP_LINE: f32 = 2.0;
pub const DROP_LINE_INSET: f32 = 3.0;
/// The columns button's side, corner and glyph, in fixed pixels.
pub const COLUMNS_BUTTON: f32 = 20.0;
pub const COLUMNS_BUTTON_RADIUS: f32 = 5.0;
pub const COLUMNS_GLYPH: f32 = 12.0;

/// What the header strip draws, resolved by the host.
pub struct ColumnHeaderStyle {
    /// The rule under the strip (PINK `hairline(0.06)`).
    pub rule: Hsla,
    /// The column names (PINK `text_faint`).
    pub text: Hsla,
}

/// The header strip, `HEADER_HEIGHT` tall over a rule. `group` names it for
/// the columns button. The host adds the graph's spacer, `commit_header`,
/// the optional headers in `cells` and `columns_button`.
pub fn column_header(
    id: impl Into<ElementId>,
    group: impl Into<SharedString>,
    style: &ColumnHeaderStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .group(group)
        .relative()
        .h(px(HEADER_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(style.rule)
        .text_size(ui_rems(ROW_TEXT))
        .text_color(style.text)
}

/// The subject column's name, taking the free width like the subject.
pub fn commit_header(text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w(px(SUBJECT_MIN_WIDTH))
        .child(text.into())
}

/// An optional column's header, `width` wide (never under `min_width`),
/// sized like its `cell`; `centered` for the avatar column. The host adds
/// the `drop_line`, the name and the `resize_handle` (last, so its narrow
/// hitbox wins over the header's drag).
pub fn header_cell(
    id: impl Into<ElementId>,
    width: f32,
    min_width: f32,
    centered: bool,
) -> Stateful<Div> {
    shrink(
        div()
            .id(id)
            .relative()
            .w(px(width))
            .min_w(px(min_width))
            .h_full(),
        1.0,
    )
    .flex()
    .items_center()
    .cursor_pointer()
    .when(centered, |el| el.justify_center())
}

/// Where a dragged header will land: a line on the cell's trailing edge
/// (`after`) or its leading one.
pub fn drop_line(after: bool, color: Hsla) -> Div {
    div()
        .absolute()
        .top(px(DROP_LINE_INSET))
        .bottom(px(DROP_LINE_INSET))
        .w(px(DROP_LINE))
        .rounded_full()
        .bg(color)
        .when(after, |line| line.right_0())
        .when(!after, |line| line.left_0())
}

/// The divider on a header's leading edge, `RESIZE_HANDLE` wide with the
/// column-resize cursor, on `fill` (already blended over its hover fade).
pub fn resize_handle(id: impl Into<ElementId>, fill: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .left(px(-RESIZE_HANDLE / 2.0))
        .top_0()
        .bottom_0()
        .w(px(RESIZE_HANDLE))
        .cursor_col_resize()
        .bg(fill)
}

/// The checklist button at the strip's trailing end. It shows while the
/// strip named `group` is pointed at, or while its menu is `open`, on
/// `wash` (already blended over its hover fade); the host adds
/// `columns_glyph`.
pub fn columns_button(
    id: impl Into<ElementId>,
    group: impl Into<SharedString>,
    open: bool,
    wash: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .right(px(3.0))
        .top(px(2.0))
        .size(px(COLUMNS_BUTTON))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(COLUMNS_BUTTON_RADIUS))
        .cursor_pointer()
        .opacity(if open { 1.0 } else { 0.0 })
        .group_hover(group, |style| style.opacity(1.0))
        .bg(wash)
}

/// The columns button's glyph.
pub fn columns_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(px(COLUMNS_GLYPH)).text_color(color)
}

// The graph cell (PINK history.rs `graph_cell`): the row's slice of the lane
// graph. The host places the node on its lane; the cell draws the node,
// HEAD's ring and a branch tip's fold control. The strokes between nodes
// are painted across every row at once (`paint_lanes` below).

/// A commit node's radius, in fixed pixels (PINK `HISTORY_NODE_RADIUS`);
/// the host grows it while its lane holds the focus.
pub const NODE_RADIUS: f32 = 3.0;
/// The gap between HEAD's node and its ring, in fixed pixels.
pub const HEAD_RING_PADDING: f32 = 2.0;
/// The fold control's side, glyph and gap after the node, in fixed pixels.
pub const FOLD_SIZE: f32 = 16.0;
pub const FOLD_GLYPH: f32 = 9.0;
pub const FOLD_GAP: f32 = 3.0;

/// The graph cell: `width` wide and `ROW_HEIGHT` tall, named `group` so a
/// branch tip's fold control shows while the cell is pointed at.
pub fn graph_cell(
    id: impl Into<ElementId>,
    group: impl Into<SharedString>,
    width: f32,
) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .group(group)
        .w(px(width))
        .h(px(ROW_HEIGHT))
        .flex_none()
}

/// The commit's node: a `radius` disc centred at `x` on the row's middle.
pub fn graph_node(x: f32, radius: f32, color: Hsla) -> Div {
    div()
        .absolute()
        .left(px(x - radius))
        .top(px(ROW_HEIGHT / 2.0 - radius))
        .size(px(radius * 2.0))
        .rounded_full()
        .bg(color)
}

/// HEAD's ring, `HEAD_RING_PADDING` outside a `radius` node at `x`: an
/// `edge` circle over `fill` (the pane's surface), drawn before the node.
pub fn head_ring(x: f32, radius: f32, edge: Hsla, fill: Hsla) -> Div {
    div()
        .absolute()
        .left(px(x - radius - HEAD_RING_PADDING))
        .top(px(ROW_HEIGHT / 2.0 - radius - HEAD_RING_PADDING))
        .size(px((radius + HEAD_RING_PADDING) * 2.0))
        .rounded_full()
        .border_1()
        .border_color(edge)
        .bg(fill)
}

/// What a fold control draws, resolved by the host from its lane's colour.
pub struct FoldStyle {
    /// The ring (PINK: the colour at 0.32).
    pub edge: Hsla,
    /// The plate (PINK: the pane's surface at 0.96).
    pub fill: Hsla,
}

/// A branch tip's fold control, `FOLD_GAP` after the node at `x`. It shows
/// while the cell named `group` is pointed at, and stays while its branch is
/// `collapsed`; the host adds `fold_glyph`.
pub fn fold_control(
    id: impl Into<ElementId>,
    group: impl Into<SharedString>,
    x: f32,
    collapsed: bool,
    style: &FoldStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .left(px(x + NODE_RADIUS + FOLD_GAP))
        .top(px((ROW_HEIGHT - FOLD_SIZE) / 2.0))
        .size(px(FOLD_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(style.edge)
        .bg(style.fill)
        .cursor_pointer()
        .opacity(if collapsed { 1.0 } else { 0.0 })
        .group_hover(group, |style| style.opacity(1.0))
}

/// The fold control's glyph (fold, or expand while collapsed).
pub fn fold_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(px(FOLD_GLYPH)).text_color(color)
}

// The lane strokes (PINK history.rs `graph_paths`): every lane through the
// visible rows, painted on the host's canvas under the rows as one stroked
// path per palette colour, so a lane runs from row to row without a seam.
// With a focused lane a second pass draws it wider over the rest, which is
// dimmed. The host lays the graph out (each row's bounds as laid out this
// frame, its segments' lane positions and colour ids, the compact rail's
// position), resolves the palette and its dimmed twin, owns the focus and
// its fade, and hit-tests the pointer against `lane_curve`.

/// A lane stroke's width, and the focused lane's at full focus, in fixed
/// pixels (PINK `HISTORY_STROKE_WIDTH`, `HISTORY_GRAPH_FOCUSED_STROKE_WIDTH`).
pub const LANE_STROKE: f32 = 1.5;
pub const LANE_FOCUSED_STROKE: f32 = 2.25;
/// How far a stroke runs past its row's top and bottom edges at the full
/// `ROW_HEIGHT`, in fixed pixels, so neighbouring rows' strokes overlap
/// instead of leaving a seam (PINK `HISTORY_GRAPH_ROW_OVERLAP`). It shrinks
/// with a row folding shut.
pub const LANE_OVERLAP: f32 = 0.75;
/// A folding row at or under this height, in pixels, draws no strokes.
pub const LANE_MIN_ROW: f32 = 0.5;

/// How a segment crosses its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneShape {
    /// Straight down through the row, staying on its lane.
    Straight,
    /// Down through the row, curving from one lane to another.
    Through,
    /// In through the row's top edge, curving onto the node.
    Incoming,
    /// Out of the node, curving through the row's bottom edge.
    Outgoing,
}

/// A segment's stroke as a cubic curve in row coordinates: pixels from the
/// row's top-left corner. A `Straight` segment is painted as the line from
/// `start` to `end`; its controls sit on that line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneCurve {
    pub start: (f32, f32),
    pub control_1: (f32, f32),
    pub control_2: (f32, f32),
    pub end: (f32, f32),
}

/// The curve a segment of `shape` strokes from the lane at `from_x` to the
/// lane at `to_x` across a row `row_height` tall. The node sits on the
/// row's middle; strokes leaving through an edge run `LANE_OVERLAP` past it,
/// scaled down while the row folds shut. The host hit-tests against the same
/// curve, so the pointer follows what is painted.
pub fn lane_curve(shape: LaneShape, from_x: f32, to_x: f32, row_height: f32) -> LaneCurve {
    let middle = row_height / 2.0;
    let overlap = LANE_OVERLAP * (row_height / ROW_HEIGHT).clamp(0.0, 1.0);
    match shape {
        LaneShape::Incoming => LaneCurve {
            start: (from_x, -overlap),
            control_1: (from_x, middle * 0.55),
            control_2: (to_x, middle * 0.55),
            end: (to_x, middle),
        },
        LaneShape::Outgoing => LaneCurve {
            start: (from_x, middle),
            control_1: (from_x, middle * 1.45),
            control_2: (to_x, middle * 1.45),
            end: (to_x, row_height + overlap),
        },
        LaneShape::Straight | LaneShape::Through => LaneCurve {
            start: (from_x, -overlap),
            control_1: (from_x, middle),
            control_2: (to_x, middle),
            end: (to_x, row_height + overlap),
        },
    }
}

/// One segment of a row's lane graph, laid out by the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneSegment {
    /// The lane it leaves and the lane it reaches, as x offsets in pixels
    /// from the row's left edge.
    pub from_x: f32,
    pub to_x: f32,
    pub shape: LaneShape,
    /// The lane's colour id: it strokes in `LaneStyle::colours[colour %
    /// len]` and is focused when it equals `LaneFocus::colour`.
    pub colour: usize,
}

/// The focused lane: its colour id and how far its focus has faded in,
/// from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneFocus {
    pub colour: usize,
    pub amount: f32,
}

/// What the lane strokes draw, already resolved by the host.
pub struct LaneStyle<'a> {
    /// The lane palette (PINK: the theme's tones, desaturated).
    pub colours: &'a [Hsla],
    /// The palette dimmed for every lane but the focused one, slot for slot,
    /// already mixed toward the pane's surface by the focus's fade (opaque,
    /// so overlapping row strokes don't stack into darker dots). Read only
    /// while `focus` is set.
    pub dimmed: &'a [Hsla],
    pub focus: Option<LaneFocus>,
}

impl LaneStyle<'_> {
    fn passes(&self) -> usize {
        if self.focus.is_some() { 2 } else { 1 }
    }

    fn width(&self, focused_pass: bool) -> f32 {
        self.focus
            .filter(|_| focused_pass)
            .map(|focus| LANE_STROKE + (LANE_FOCUSED_STROKE - LANE_STROKE) * focus.amount)
            .unwrap_or(LANE_STROKE)
    }

    // Whether a stroke of colour id `colour` belongs to palette `slot` in
    // this pass: the focused lane draws only in the second pass, the rest
    // only in the first.
    fn takes(&self, colour: usize, slot: usize, focused_pass: bool) -> bool {
        colour % self.colours.len() == slot
            && self
                .focus
                .is_none_or(|focus| (colour == focus.colour) == focused_pass)
    }

    fn paint_colour(&self, slot: usize, focused_pass: bool) -> Hsla {
        let colour = self.colours[slot];
        if self.focus.is_some() && !focused_pass {
            self.dimmed.get(slot).copied().unwrap_or(colour)
        } else {
            colour
        }
    }
}

// A row the strokes reach: inside the viewport and not folded shut.
fn lane_row_drawn(bounds: &Bounds<Pixels>, viewport: &Bounds<Pixels>) -> bool {
    !(bounds.bottom() < viewport.top() || bounds.top() > viewport.bottom())
        && f32::from(bounds.size.height) > LANE_MIN_ROW
}

/// Paints every lane through the rows that reach `viewport`: per pass and
/// palette slot, one stroked path holding that colour's segments from every
/// row, the focused lane last and wider. `rows` yields each laid-out row's
/// bounds with its segments, top to bottom; rows outside the viewport or
/// folded shut are skipped without reading their segments.
pub fn paint_lanes<R, S>(window: &mut Window, viewport: Bounds<Pixels>, rows: R, style: &LaneStyle)
where
    R: IntoIterator<Item = (Bounds<Pixels>, S)>,
    S: IntoIterator<Item = LaneSegment>,
{
    if style.colours.is_empty() {
        return;
    }
    let rows: Vec<(Bounds<Pixels>, Vec<LaneSegment>)> = rows
        .into_iter()
        .filter(|(bounds, _)| lane_row_drawn(bounds, &viewport))
        .map(|(bounds, segments)| (bounds, segments.into_iter().collect()))
        .collect();
    for pass in 0..style.passes() {
        let focused_pass = style.focus.is_some() && pass == 1;
        for slot in 0..style.colours.len() {
            let mut builder = PathBuilder::stroke(px(style.width(focused_pass)));
            let mut any = false;
            for (bounds, segments) in &rows {
                let origin = bounds.origin;
                let at = |(x, y): (f32, f32)| point(origin.x + px(x), origin.y + px(y));
                let height = f32::from(bounds.size.height);
                for segment in segments
                    .iter()
                    .filter(|segment| style.takes(segment.colour, slot, focused_pass))
                {
                    any = true;
                    let curve = lane_curve(segment.shape, segment.from_x, segment.to_x, height);
                    builder.move_to(at(curve.start));
                    if segment.shape == LaneShape::Straight {
                        builder.line_to(at(curve.end));
                    } else {
                        builder.cubic_bezier_to(
                            at(curve.end),
                            at(curve.control_1),
                            at(curve.control_2),
                        );
                    }
                }
            }
            if any && let Ok(path) = builder.build() {
                window.paint_path(path, style.paint_colour(slot, focused_pass));
            }
        }
    }
}

/// One row of the compact rail, laid out by the host.
pub struct RailRow {
    /// The row's bounds as laid out this frame.
    pub bounds: Bounds<Pixels>,
    /// The next row's bounds, where the rail meets its node; `None` for
    /// the last row, whose rail runs to its bottom edge.
    pub next: Option<Bounds<Pixels>>,
    /// The row's node colour id.
    pub colour: usize,
}

/// Paints the compact rail: when the graph is too narrow for its lanes,
/// one vertical line at `rail_x` (pixels from each row's left edge) runs
/// from each node to the next in the node's colour, batched and focused
/// like `paint_lanes`.
pub fn paint_rail<R>(
    window: &mut Window,
    viewport: Bounds<Pixels>,
    rail_x: f32,
    rows: R,
    style: &LaneStyle,
) where
    R: IntoIterator<Item = RailRow>,
{
    if style.colours.is_empty() {
        return;
    }
    let rows: Vec<RailRow> = rows
        .into_iter()
        .filter(|row| lane_row_drawn(&row.bounds, &viewport))
        .collect();
    let middle = |bounds: &Bounds<Pixels>| -> Point<Pixels> {
        point(
            bounds.origin.x + px(rail_x),
            bounds.origin.y + px(f32::from(bounds.size.height) / 2.0),
        )
    };
    for pass in 0..style.passes() {
        let focused_pass = style.focus.is_some() && pass == 1;
        for slot in 0..style.colours.len() {
            let mut builder = PathBuilder::stroke(px(style.width(focused_pass)));
            let mut any = false;
            for row in rows
                .iter()
                .filter(|row| style.takes(row.colour, slot, focused_pass))
            {
                let end_y = row
                    .next
                    .map(|next| middle(&next).y)
                    .unwrap_or_else(|| row.bounds.bottom());
                builder.move_to(middle(&row.bounds));
                builder.line_to(point(row.bounds.origin.x + px(rail_x), end_y));
                any = true;
            }
            if any && let Ok(path) = builder.build() {
                window.paint_path(path, style.paint_colour(slot, focused_pass));
            }
        }
    }
}
