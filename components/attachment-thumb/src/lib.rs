// PINK's attachment thumbnails, two sizes:
//
// - Sent (transcript.rs `render_user_attachments`): the 112 by 80 tiles in
//   the right-aligned strip above a user message. A loaded image covers a
//   hairline plate; while the message is still crossing, a black veil that
//   breathes on the pulse dims it under the host's upload ring or spinner;
//   a file that could not load leaves a dashed "missing" tile; one still
//   loading pulses as a skeleton.
// - Staged (composer.rs `render_attachment_strip`): the 56 pt squares in the
//   wrapping strip above the composer's input, each with a raised remove
//   disc overhanging its top right corner, shown on hover and focus.
//
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision. The package draws only. The host supplies the resolved colours,
// the image, the pulse (0 to 1 on its shared clock) and the indicator; it
// adds the ids, the pointer, the role and accessible names, the focus
// handling, the hover group that reveals the remove disc, its glyph and
// frosted layer, and the clicks that open the lightbox or remove the file.
// Plain `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{Div, Hsla, ImageSource, Img, ObjectFit, div, hsla, img, prelude::*, px};

/// The sent tile's size, in px.
pub const SENT_WIDTH: f32 = 112.0;
pub const SENT_HEIGHT: f32 = 80.0;
/// The staged square's side, in px.
pub const STAGED_SIZE: f32 = 56.0;
/// Every tile's corners, in px.
pub const RADIUS: f32 = 8.0;
/// The image's own corners inside the 1 px edge. The frame's rounding only
/// clips rectangularly, so the image rounds itself: the staged square uses
/// the frame's 8 minus its edge, the sent tile the frame's 8.
pub const SENT_IMAGE_RADIUS: f32 = 8.0;
pub const STAGED_IMAGE_RADIUS: f32 = 7.0;
/// The sending veil: black at this strength, breathing up by
/// `SENDING_PULSE` on the host's pulse.
pub const SENDING_ALPHA: f32 = 0.38;
pub const SENDING_PULSE: f32 = 0.05;
/// The loading skeleton's opacity: `SKELETON_REST` plus `SKELETON_PULSE`
/// on the host's pulse.
pub const SKELETON_REST: f32 = 0.35;
pub const SKELETON_PULSE: f32 = 0.4;
/// The host's indicators on the sending veil: the upload ring's diameter
/// and the indeterminate spinner's cell, in px.
pub const RING: f32 = 34.0;
pub const SPINNER_CELL: f32 = 3.0;
/// The sent strip: the gap between tiles and its padding, in px.
pub const SENT_STRIP_GAP: f32 = 8.0;
pub const SENT_STRIP_PAD_X: f32 = 4.0;
pub const SENT_STRIP_PAD_TOP: f32 = 4.0;
pub const SENT_STRIP_PAD_BOTTOM: f32 = 6.0;
/// The staged strip: the gap between squares and its padding, in px.
pub const STAGED_STRIP_GAP: f32 = 8.0;
pub const STAGED_STRIP_PAD_X: f32 = 16.0;
pub const STAGED_STRIP_PAD_TOP: f32 = 12.0;
/// The remove disc, how far it overhangs the square's top right corner
/// and its glyph, in px.
pub const REMOVE_SIZE: f32 = 20.0;
pub const REMOVE_OVERHANG: f32 = 7.0;
pub const REMOVE_GLYPH: f32 = 10.0;

/// Which tile: the sent tile above a message or the staged square above the
/// composer's input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sent,
    Staged,
}

impl Kind {
    pub fn width(self) -> f32 {
        match self {
            Kind::Sent => SENT_WIDTH,
            Kind::Staged => STAGED_SIZE,
        }
    }

    pub fn height(self) -> f32 {
        match self {
            Kind::Sent => SENT_HEIGHT,
            Kind::Staged => STAGED_SIZE,
        }
    }

    fn image_radius(self) -> f32 {
        match self {
            Kind::Sent => SENT_IMAGE_RADIUS,
            Kind::Staged => STAGED_IMAGE_RADIUS,
        }
    }
}

/// What a tile's edge and plate draw, already resolved by the host.
pub struct PlateStyle {
    /// The 1 px edge.
    pub edge: Hsla,
    /// The plate under the image (transparent where the host draws none).
    pub fill: Hsla,
}

/// The tile's bare frame: its size and corners, clipping what it holds.
pub fn frame(kind: Kind) -> Div {
    div()
        .flex_none()
        .w(px(kind.width()))
        .h(px(kind.height()))
        .rounded(px(RADIUS))
        .overflow_hidden()
}

/// A tile holding an image: the frame on its edged plate, positioned for the
/// [`sending`] veil. The host adds the id, the pointer, the click that opens
/// the lightbox, the [`image`] and, while it crosses, the veil.
pub fn plate(kind: Kind, style: &PlateStyle) -> Div {
    frame(kind)
        .relative()
        .border_1()
        .border_color(style.edge)
        .bg(style.fill)
}

/// The image covering the tile inside its edge, with explicit dimensions:
/// an image honours its own aspect ratio over a percentage height, so a tall
/// photo would otherwise grow past the frame.
pub fn image(kind: Kind, source: impl Into<ImageSource>) -> Img {
    img(source)
        .w(px(kind.width() - 2.0))
        .h(px(kind.height() - 2.0))
        .rounded(px(kind.image_radius()))
        .object_fit(ObjectFit::Cover)
}

/// The veil over a tile whose bytes are still crossing, at `pulse` (0 to 1)
/// on the host's clock. The host adds its indicator: the upload ring
/// (`RING`) or the spinner (`SPINNER_CELL`).
pub fn sending(pulse: f32) -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(px(RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .bg(hsla(0.0, 0.0, 0.0, SENDING_ALPHA + SENDING_PULSE * pulse))
}

/// The dashed tile left where a file could not load.
pub fn missing(kind: Kind, style: &PlateStyle) -> Div {
    frame(kind)
        .border_1()
        .border_dashed()
        .border_color(style.edge)
        .bg(style.fill)
}

/// The tile while its image loads, breathing at `pulse` (0 to 1) on the
/// host's clock.
pub fn skeleton(kind: Kind, style: &PlateStyle, pulse: f32) -> Div {
    frame(kind)
        .border_1()
        .border_color(style.edge)
        .bg(style.fill)
        .opacity(SKELETON_REST + SKELETON_PULSE * pulse)
}

/// The right-aligned, wrapping strip of sent tiles above a user message.
pub fn sent_strip() -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex_none()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_end()
        .items_start()
        .gap(px(SENT_STRIP_GAP))
        .px(px(SENT_STRIP_PAD_X))
        .pt(px(SENT_STRIP_PAD_TOP))
        .pb(px(SENT_STRIP_PAD_BOTTOM))
}

/// The wrapping strip of staged squares above the composer's input.
pub fn staged_strip() -> Div {
    div()
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(STAGED_STRIP_GAP))
        .px(px(STAGED_STRIP_PAD_X))
        .pt(px(STAGED_STRIP_PAD_TOP))
}

/// What the remove disc draws, already resolved by the host.
pub struct RemoveStyle {
    /// The raised disc.
    pub fill: Hsla,
    /// Its 1 px edge.
    pub edge: Hsla,
}

/// The remove disc overhanging a staged square's top right corner, hidden
/// until the host's hover group or keyboard focus raises its opacity. Place
/// the square and the disc in one relatively positioned slot. The host adds
/// the id, the role and name, the tab stop, the hover and focus styles, the
/// pointer, the click and the glyph (`REMOVE_GLYPH` px).
pub fn remove_disc(style: &RemoveStyle) -> Div {
    div()
        .absolute()
        .top(px(-REMOVE_OVERHANG))
        .right(px(-REMOVE_OVERHANG))
        .size(px(REMOVE_SIZE))
        .rounded_full()
        .bg(style.fill)
        .border_1()
        .border_color(style.edge)
        .flex()
        .items_center()
        .justify_center()
        .shadow_sm()
        .opacity(0.0)
}
