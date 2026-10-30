// The memory panel's pieces (the gallery's Memory Panel story, adopted by
// PINK's Providers settings for the selected chat's provider memory): a
// memory row with its faded title and quiet meta line, the corner actions
// that appear under the pointer, the section heading over a group of rows,
// the note strip that folds open under the header (paused, applies at next
// launch, an error), the list frame and the footer bar.
//
// The package draws only. The host supplies the colours (each row's wash
// already blended over its hover fade, the fade's target flattened over the
// card), the glyph paths and the text; it adds the ids, the hover
// listeners, the actions, the text inputs, the tooltips, the focus and the
// accessibility. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, AnyElement, Div, FontWeight, Hsla, SharedString, div, linear_color_stop,
    linear_gradient, prelude::*, rems, svg,
};

/// A memory row's height: a title line over one meta line (PINK's sidebar
/// chat row), in pt at the 16 px default.
pub const ROW_H: f32 = 52.0;
/// A row's horizontal padding.
pub const ROW_PAD_X: f32 = 8.0;
/// A row's vertical padding.
pub const ROW_PAD_Y: f32 = 6.0;
/// A row's corner radius.
pub const ROW_RADIUS: f32 = 8.0;
/// The title's size and line height.
pub const TITLE_TEXT: f32 = 13.0;
pub const TITLE_LINE: f32 = 18.0;
/// The meta line's size and line height.
pub const META_TEXT: f32 = 11.0;
pub const META_LINE: f32 = 14.0;
/// The glyph beside the meta text.
pub const META_GLYPH: f32 = 11.0;
/// Only the title's last few glyphs recede instead of ending in an ellipsis.
pub const TITLE_FADE: f32 = 20.0;
/// A corner action: its square (the minimum hit target), its glyph and its
/// corner.
pub const ACTION: f32 = 24.0;
pub const ACTION_GLYPH: f32 = 12.0;
pub const ACTION_RADIUS: f32 = 4.0;
/// A section heading's height, and the gap above every heading but the first.
pub const HEADING_H: f32 = 28.0;
pub const SECTION_GAP: f32 = 12.0;
/// The list's side inset: with a row's own padding it puts every memory on
/// the card header's 16 pt edge.
pub const LIST_INSET: f32 = 8.0;
/// The note strip's side margin, padding and corner.
pub const STRIP_MARGIN_X: f32 = 16.0;
pub const STRIP_PAD_X: f32 = 16.0;
pub const STRIP_PAD_Y: f32 = 10.0;
pub const STRIP_RADIUS: f32 = 12.0;
/// The footer bar: a small control with 10 pt above and below.
pub const FOOTER_H: f32 = 44.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a memory row draws, each colour already resolved by the host.
pub struct RowStyle {
    /// The title, blended toward full strength under the pointer.
    pub title: Hsla,
    /// The meta line and its glyph.
    pub meta: Hsla,
    /// The row's wash over its hover fade (transparent at rest).
    pub wash: Hsla,
    /// The opaque colour the title fades into: the wash flattened over the
    /// card.
    pub fade_to: Hsla,
}

/// A memory row: the title line over the meta line, `ROW_H` tall, padded
/// and rounded. The host chains `title_line` and `meta_line` as children and
/// adds the id, role, hover listener and click.
pub fn row(style: &RowStyle) -> Div {
    div()
        .h(ui_rems(ROW_H))
        .flex_none()
        .px(ui_rems(ROW_PAD_X))
        .py(ui_rems(ROW_PAD_Y))
        .flex()
        .flex_col()
        .justify_center()
        .gap(ui_rems(2.0))
        .rounded(ui_rems(ROW_RADIUS))
        .bg(style.wash)
        .text_color(style.title)
}

/// The title line: the memory on one line that fades out at its right edge
/// into `fade_to`, then the host's corner actions (shown under the pointer).
pub fn title_line(
    fade_to: Hsla,
    title: impl Into<SharedString>,
    corner: Option<AnyElement>,
) -> Div {
    let text = div()
        .flex_1()
        .min_w_0()
        .relative()
        .overflow_hidden()
        .flex()
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_size(ui_rems(TITLE_TEXT))
                .line_height(ui_rems(TITLE_LINE))
                .child(title.into()),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(ui_rems(TITLE_FADE))
                .bg(linear_gradient(
                    90.,
                    linear_color_stop(fade_to.opacity(0.), 0.),
                    linear_color_stop(fade_to, 1.),
                )),
        );
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(ui_rems(8.0))
        .child(text)
        .children(corner)
}

/// The corner beside the title: the host's actions, `opacity` following the
/// row's hover fade.
pub fn corner(opacity: f32) -> Div {
    div()
        .flex_none()
        .h(ui_rems(TITLE_LINE))
        .flex()
        .items_center()
        .gap(ui_rems(2.0))
        .opacity(opacity)
}

/// One corner action: `ACTION` square, `ACTION_RADIUS` corners and a
/// `ACTION_GLYPH` glyph in `tone`. The host adds the id, role, label,
/// hover wash, tooltip and click.
pub fn action(icon: impl Into<SharedString>, tone: Hsla) -> Div {
    div()
        .size(ui_rems(ACTION))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(ui_rems(ACTION_RADIUS))
        .cursor_pointer()
        .child(
            svg()
                .path(icon.into())
                .size(ui_rems(ACTION_GLYPH))
                .flex_none()
                .text_color(tone),
        )
}

/// The quiet meta line under the title: an optional glyph, then where the
/// memory came from, truncating.
pub fn meta_line(color: Hsla, glyph: Option<SharedString>, text: impl Into<SharedString>) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(ui_rems(4.0))
        .text_size(ui_rems(META_TEXT))
        .line_height(ui_rems(META_LINE))
        .text_color(color)
        .when_some(glyph, |d, path| {
            d.child(
                svg()
                    .path(path)
                    .size(ui_rems(META_GLYPH))
                    .flex_none()
                    .text_color(color),
            )
        })
        .child(div().flex_1().min_w_0().truncate().child(text.into()))
}

/// A section's heading over its rows: `HEADING_H` tall, the label in 11
/// semibold `label_color` and its count in `count_color`. `gap` is the space
/// above (`SECTION_GAP` for every section after the first).
pub fn heading(
    label_color: Hsla,
    count_color: Hsla,
    label: impl Into<SharedString>,
    count: usize,
    gap: f32,
) -> Div {
    div().pt(ui_rems(gap)).flex_none().child(
        div()
            .h(ui_rems(HEADING_H))
            .px(ui_rems(ROW_PAD_X))
            .flex()
            .items_center()
            .gap(ui_rems(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(ui_rems(META_TEXT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(label_color)
                    .child(label.into()),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(ui_rems(META_TEXT))
                    .text_color(count_color)
                    .child(count.to_string()),
            ),
    )
}

/// What a note strip draws: its plate, edge and copy colours.
pub struct StripStyle {
    pub plate: Hsla,
    pub edge: Hsla,
    pub copy: Hsla,
}

/// A note strip under the header (memory paused, a change that applies at
/// the next launch, an error): a bordered plate inset `STRIP_MARGIN_X`, its
/// glyph beside wrapping 12/16 copy.
pub fn strip(
    style: &StripStyle,
    icon: impl Into<SharedString>,
    text: impl Into<SharedString>,
) -> Div {
    div()
        .mx(ui_rems(STRIP_MARGIN_X))
        .flex()
        .items_start()
        .gap(ui_rems(8.0))
        .px(ui_rems(STRIP_PAD_X))
        .py(ui_rems(STRIP_PAD_Y))
        .rounded(ui_rems(STRIP_RADIUS))
        .border_1()
        .border_color(style.edge)
        .bg(style.plate)
        .text_size(ui_rems(12.0))
        .line_height(ui_rems(16.0))
        .text_color(style.copy)
        .child(
            svg()
                .path(icon.into())
                .size(ui_rems(14.0))
                .mt(ui_rems(1.0))
                .flex_none()
                .text_color(style.copy),
        )
        .child(div().flex_1().min_w_0().child(text.into()))
}

/// The list under the header: its rows inset `LIST_INSET` below a rule in
/// `border`.
pub fn list(border: Hsla) -> Div {
    div()
        .flex()
        .flex_col()
        .px(ui_rems(LIST_INSET))
        .py(ui_rems(6.0))
        .border_t_1()
        .border_color(border)
}

/// The footer bar below a rule in `border`: the host's summary or Undo on
/// the left and its action on the right.
pub fn footer(border: Hsla) -> Div {
    div()
        .min_h(ui_rems(FOOTER_H))
        .flex_none()
        .flex()
        .items_center()
        .gap(ui_rems(8.0))
        .pl(ui_rems(16.0))
        .pr(ui_rems(10.0))
        .border_t_1()
        .border_color(border)
}

/// The footer's quiet summary ("8 memories, 2 pinned"), truncating.
pub fn summary(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(META_TEXT))
        .line_height(ui_rems(META_LINE))
        .text_color(color)
        .child(text.into())
}
