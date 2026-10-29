// PINK's message queue (queue.rs `queue_panel_surface`, `queue_row`,
// `queue_thumbnail`, `queue_action`, `queue_primary_action_button`): the
// tray tucked behind the composer and the one row shape its queued messages
// share, with the drag marker, the message and its status, image
// thumbnails, the always-visible Remove and Edit discs and the one primary
// delivery action. UI rethink decision 54: drawn here, consumed by PINK
// through a pinned revision.
//
// The package draws only. The host supplies the resolved colours (washes
// and labels already blended over their hover fades), the tray's shadow
// (none on a frosted surface), the glyph paths and the copy; it adds the
// ids, the hover listeners, roles and accessible names, focus rings,
// tooltips, drag and drop, the slide animation, scrolling and the actions.
// Plain `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{
    AbsoluteLength, BoxShadow, CursorStyle, Div, FontWeight, Hsla, ImageSource, Img, ObjectFit,
    SharedString, Svg, div, img, prelude::*, px, rems, svg,
};

/// The tray's top corners: the stack ladder's tray step (PINK popover.rs
/// `STACK_TRAY_RADIUS`).
pub const TRAY_RADIUS: f32 = 18.0;
/// One row's height. Rows sit flush, without a gap.
pub const ROW_HEIGHT: f32 = 36.0;
pub const ROW_PAD_X: f32 = 8.0;
pub const ROW_RADIUS: f32 = 8.0;
/// The gap between a row's parts; a narrow composer that previews one
/// thumbnail tightens it.
pub const ROW_GAP: f32 = 8.0;
pub const COMPACT_ROW_GAP: f32 = 4.0;
/// A row being removed fades to this while the host confirms it.
pub const REMOVING_OPACITY: f32 = 0.55;
/// The message line, in pt at the 16 px default.
pub const TEXT: f32 = 12.5;
pub const LINE_HEIGHT: f32 = 16.0;
/// The status in front of the message.
pub const STATUS_GAP: f32 = 6.0;
/// The message line over the attachment summary.
pub const LINE_GAP: f32 = 1.0;
pub const SUMMARY_TEXT: f32 = 11.0;
pub const SUMMARY_LINE_HEIGHT: f32 = 13.0;
/// The queue's glyphs: the drag marker and the row actions.
pub const GLYPH: f32 = 13.0;
/// The drag marker: a 20 by 28 hit area for its glyph.
pub const MARKER_WIDTH: f32 = 20.0;
pub const MARKER_HEIGHT: f32 = 28.0;
pub const MARKER_RADIUS: f32 = 4.0;
/// A marker whose row can't be reordered right now.
pub const BLOCKED_MARKER_OPACITY: f32 = 0.35;
/// Holds the text on its column while an edited row drops its marker.
pub const EDIT_SPACER: f32 = 14.0;
/// The row's actions: 28 pt discs, 3 apart.
pub const ACTION_SIZE: f32 = 28.0;
pub const ACTION_GAP: f32 = 3.0;
/// The primary delivery action: wide enough for either label, so a
/// modifier preview never moves the row; a disc in a compact row.
pub const PRIMARY_WIDTH: f32 = 72.0;
pub const PRIMARY_TEXT: f32 = 12.0;
pub const DISABLED_OPACITY: f32 = 0.45;
/// An image thumbnail's frame and the image inset inside its hairline.
pub const THUMB_WIDTH: f32 = 40.0;
pub const THUMB_HEIGHT: f32 = 28.0;
pub const THUMB_RADIUS: f32 = 5.0;
pub const THUMB_IMAGE_RADIUS: f32 = 4.0;
/// The glyph a thumbnail shows while its preview is loading or failed.
pub const THUMB_GLYPH: f32 = 14.0;
/// The count of attachments past the previews.
pub const MORE_SIZE: f32 = 28.0;
pub const MORE_TEXT: f32 = 11.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the tray draws.
pub struct TrayStyle {
    pub background: Hsla,
    pub border: Hsla,
    /// The tray's lift; `None` on a frosted surface.
    pub shadows: Option<Vec<BoxShadow>>,
    /// How far the composer overlaps the tray's foot: only that part is
    /// padded, so the visible rows stay flush with the tray.
    pub tucked: f32,
}

/// The tray: rounded at the top, bordered, its rows stacked in a column.
/// It occludes what is under it; the host adds the rows' scroller.
pub fn tray(style: &TrayStyle) -> Div {
    let tray = div()
        .occlude()
        .rounded_t(px(TRAY_RADIUS))
        .bg(style.background)
        .border_1()
        .border_color(style.border);
    let tray = match &style.shadows {
        Some(shadows) => tray.shadow(shadows.clone()),
        None => tray,
    };
    tray.pb(px(style.tucked)).flex().flex_col()
}

/// What one row draws.
pub struct RowStyle {
    /// The wash: the edited row's fill, or the hover wash already blended
    /// over its fade (transparent at rest).
    pub fill: Hsla,
    /// The tighter gap of a composer that previews one thumbnail.
    pub compact: bool,
    /// Faded while its removal is confirmed.
    pub removing: bool,
}

/// One queued message's row: its parts in a line, padded and rounded.
pub fn row(style: &RowStyle) -> Div {
    div()
        .h(px(ROW_HEIGHT))
        .flex_none()
        .px(px(ROW_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(if style.compact {
            COMPACT_ROW_GAP
        } else {
            ROW_GAP
        }))
        .rounded(px(ROW_RADIUS))
        .bg(style.fill)
        .when(style.removing, |row| row.opacity(REMOVING_OPACITY))
        .cursor(CursorStyle::Arrow)
}

/// The quiet drag marker that says the row belongs to the queue. A row
/// that can't be reordered dims it and drops the pointing cursor.
pub fn drag_marker(glyph: impl Into<SharedString>, color: Hsla, blocked: bool) -> Div {
    div()
        .w(px(MARKER_WIDTH))
        .h(px(MARKER_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(MARKER_RADIUS))
        .cursor_pointer()
        .when(blocked, |marker| {
            marker
                .cursor(CursorStyle::Arrow)
                .opacity(BLOCKED_MARKER_OPACITY)
        })
        .child(
            svg()
                .path(glyph.into())
                .flex_none()
                .size(px(GLYPH))
                .text_color(color),
        )
}

/// The marker's place while the row is edited, so the text stays put.
pub fn edit_spacer() -> Div {
    div().w(px(EDIT_SPACER)).flex_none()
}

/// The text column: the message line over an optional summary line.
pub fn content() -> Div {
    div().flex_1().min_w_0().flex().flex_col().gap(px(LINE_GAP))
}

/// The message line, in the host's text colour: an optional status, then
/// the message, which alone truncates.
pub fn message_line(text: Hsla) -> Div {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(STATUS_GAP))
        .text_size(ui_rems(TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(text)
}

/// The delivery status that leads a claimed, steered or held message, in a
/// quieter tone.
pub fn status(color: Hsla, label: impl Into<SharedString>) -> Div {
    div().flex_none().text_color(color).child(label.into())
}

/// The message itself, on one line.
pub fn message(text: impl Into<SharedString>) -> Div {
    div().min_w_0().truncate().child(text.into())
}

/// The attachment summary under the message.
pub fn summary(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .truncate()
        .text_size(ui_rems(SUMMARY_TEXT))
        .line_height(ui_rems(SUMMARY_LINE_HEIGHT))
        .text_color(color)
        .child(text.into())
}

/// What an edited row says in place of its message.
pub fn editing_note(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .text_size(ui_rems(TEXT))
        .text_color(color)
        .child(text.into())
}

/// The trailing group of actions.
pub fn actions() -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(ACTION_GAP))
}

/// What a row action draws: its wash and label colour, each already
/// blended over the hover fade, and whether it can act.
pub struct ActionStyle {
    pub fill: Hsla,
    pub text: Hsla,
    pub enabled: bool,
}

/// A permanently visible row action (Remove, Edit, Save, Cancel): a 28 pt
/// disc that holds its glyph; dimmed and without the pointing cursor while
/// it can't act.
pub fn action(style: &ActionStyle) -> Div {
    div()
        .size(px(ACTION_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(style.fill)
        .text_color(style.text)
        .when(style.enabled, |action| action.cursor_pointer())
        .when(!style.enabled, |action| {
            action.cursor(CursorStyle::Arrow).opacity(DISABLED_OPACITY)
        })
}

/// A row action's glyph, in the host's colour. The host brightens it with
/// the action's hover group.
pub fn action_glyph(glyph: impl Into<SharedString>, color: Hsla) -> Svg {
    svg()
        .path(glyph.into())
        .flex_none()
        .size(px(GLYPH))
        .text_color(color)
}

/// The row's one delivery action (Send now or Steer now): a 72 by 28
/// capsule with a medium label, or a disc in a compact row that holds the
/// host's send glyph. Dimmed while blocked.
pub fn primary(style: &ActionStyle, compact: bool) -> Div {
    div()
        .w(px(if compact { ACTION_SIZE } else { PRIMARY_WIDTH }))
        .h(px(ACTION_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_size(ui_rems(PRIMARY_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .bg(style.fill)
        .text_color(style.text)
        .when(style.enabled, |primary| primary.cursor_pointer())
        .when(!style.enabled, |primary| primary.opacity(DISABLED_OPACITY))
}

/// What an image thumbnail's frame draws: its hairline (blended over the
/// hover fade when it opens a preview) and its backing.
pub struct ThumbnailStyle {
    pub border: Hsla,
    pub background: Hsla,
}

/// An image thumbnail's frame, clipping what it holds.
pub fn thumbnail(style: &ThumbnailStyle) -> Div {
    div()
        .w(px(THUMB_WIDTH))
        .h(px(THUMB_HEIGHT))
        .flex_none()
        .rounded(px(THUMB_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .overflow_hidden()
}

/// The decoded preview inside a [`thumbnail`], filling it inside the
/// hairline.
pub fn thumbnail_image(source: impl Into<ImageSource>) -> Img {
    img(source)
        .w(px(THUMB_WIDTH - 2.0))
        .h(px(THUMB_HEIGHT - 2.0))
        .rounded(px(THUMB_IMAGE_RADIUS))
        .object_fit(ObjectFit::Cover)
}

/// A [`thumbnail`] whose preview is loading or failed: the host's glyph
/// centred in the frame.
pub fn thumbnail_placeholder(style: &ThumbnailStyle, glyph: impl Into<SharedString>) -> Div {
    thumbnail(style)
        .flex()
        .items_center()
        .justify_center()
        .child(svg().path(glyph.into()).flex_none().size(px(THUMB_GLYPH)))
}

/// How many attachments the previews leave out, as `+N`.
pub fn more(background: Hsla, text: Hsla, label: impl Into<SharedString>) -> Div {
    div()
        .w(px(MORE_SIZE))
        .h(px(MORE_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(THUMB_RADIUS))
        .bg(background)
        .text_size(ui_rems(MORE_TEXT))
        .text_color(text)
        .child(label.into())
}
