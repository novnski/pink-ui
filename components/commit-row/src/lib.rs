// PINK's commit history rows (history.rs `render_row`, `render_ref`,
// `render_ref_area`, `render_author_cell`, `render_date_cell`,
// `render_sha_cell` and the Load more row): one commit as a `ROW_HEIGHT` row
// after the host's graph cell, with its subject and ref badges (branch,
// remote, tag; an overflow count past the room), then the optional author
// (avatar or name), date and short-sha cells; and the button that loads
// older commits. UI rethink decision 54: drawn here, consumed by PINK through
// a pinned revision.
//
// The package draws only. The host supplies the graph cell, the glyphs and
// avatar images (its own assets), every colour already resolved (the row's
// and the sha's washes blended over their hover fades, the graph focus's
// dimming as an opacity), the families and the text; it adds the ids'
// listeners, tooltips, the container query that sizes the ref area, column
// widths and their persistence, the fold transition, copying a sha and
// opening a commit. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, ElementId, Hsla, Img, ObjectFit, SharedString, Stateful, StyledImage, Svg,
    div, prelude::*, px, rems,
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
