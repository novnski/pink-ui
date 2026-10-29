// PINK's Markdown lists (markdown/render.rs `render_block`, `Block::List`):
// items stack 4 apart, each a row of an 18 pt marker column 8 from the
// item's blocks. Markers wear the accent: a real 5 pt disc centred on the
// first line (the glyph "•" reads too small at body size), a tinted number,
// or a task checkbox. A task item's box replaces its `[ ]` / `[x]` text:
// 16 pt, r4, a 1 pt edge in the open tone, or an accent plate holding a
// 12 pt check when done. A box the reader cannot toggle (an assistant reply)
// rests at half strength.
//
// The package draws only. The host passes resolved colours, its body text
// metrics and its check icon, and adds the id, the CheckBox role, the
// toggled state and accessible name, the focus ring, the cursor and the
// click. Sizes are rems, so lists follow the host's Interface size. Plain
// `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, IntoElement, SharedString, Svg, div, prelude::*, px, rems};

/// Space between items, and between the blocks of one item.
pub const ITEM_GAP: f32 = 4.0;
/// The marker column's minimum width.
pub const MARKER_MIN_W: f32 = 18.0;
/// Space between the marker column and the item's blocks.
pub const MARKER_GAP: f32 = 8.0;
/// The bullet disc's diameter.
pub const DISC: f32 = 5.0;
/// The disc's inset from the column's leading edge, in px.
pub const DISC_INSET: f32 = 1.0;
/// The task checkbox's side.
pub const TASK_BOX: f32 = 16.0;
/// The check drawn inside a done task's box.
pub const TASK_CHECK: f32 = 12.0;
/// The task checkbox's corner radius, in px.
pub const TASK_RADIUS: f32 = 4.0;
/// A box the reader cannot toggle rests at this opacity.
pub const READ_ONLY_OPACITY: f32 = 0.5;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a task checkbox draws.
pub struct TaskStyle {
    /// The edge of an open task's box.
    pub edge: Hsla,
    /// A done task's plate and edge (the accent).
    pub plate: Hsla,
    /// The check on the plate (the canvas colour).
    pub check: Hsla,
}

/// The list: its items stacked `ITEM_GAP` apart.
pub fn list() -> Div {
    div().flex().flex_col().gap(ui_rems(ITEM_GAP))
}

/// One item: its `marker` column, then its `body` `MARKER_GAP` to the right.
pub fn item(marker: impl IntoElement, body: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_row()
        .gap(ui_rems(MARKER_GAP))
        .child(marker)
        .child(body)
}

/// The item's blocks, stacked `ITEM_GAP` apart and free to shrink, so long
/// text wraps beside the marker instead of under it.
pub fn body() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(ui_rems(ITEM_GAP))
}

/// A marker column one body line tall (`line_height`, pt) that centres what
/// it holds on that line: the disc or a task box.
pub fn marker_slot(line_height: f32) -> Div {
    div()
        .flex_none()
        .min_w(ui_rems(MARKER_MIN_W))
        .h(ui_rems(line_height))
        .flex()
        .items_center()
}

/// An unordered item's marker: a `DISC` in `color` centred on the first
/// line.
pub fn disc(color: Hsla, line_height: f32) -> Div {
    marker_slot(line_height).child(
        div()
            .ml(px(DISC_INSET))
            .size(ui_rems(DISC))
            .rounded_full()
            .bg(color),
    )
}

/// An ordered item's marker: `label` ("3.") in `color` at the body's
/// `text_size` / `line_height` (pt).
pub fn number(
    label: impl Into<SharedString>,
    color: Hsla,
    text_size: f32,
    line_height: f32,
) -> Div {
    div()
        .flex_none()
        .min_w(ui_rems(MARKER_MIN_W))
        .text_size(ui_rems(text_size))
        .line_height(ui_rems(line_height))
        .text_color(color)
        .child(label.into())
}

/// A task's box: `TASK_BOX` with a 1 px edge, open in `style.edge`, done as
/// a `style.plate` holding the host's `check` icon at `TASK_CHECK` in
/// `style.check`. A box the reader cannot toggle (`toggleable` false) rests
/// at `READ_ONLY_OPACITY`. Place it in [`marker_slot`].
pub fn task_box(style: &TaskStyle, checked: bool, toggleable: bool, check: Svg) -> Div {
    div()
        .flex_none()
        .size(ui_rems(TASK_BOX))
        .border_1()
        .rounded(px(TASK_RADIUS))
        .border_color(if checked { style.plate } else { style.edge })
        .when(checked, |el| el.bg(style.plate))
        .flex()
        .items_center()
        .justify_center()
        .when(!toggleable, |el| el.opacity(READ_ONLY_OPACITY))
        .when(checked, |el| {
            el.child(check.size(ui_rems(TASK_CHECK)).text_color(style.check))
        })
}
