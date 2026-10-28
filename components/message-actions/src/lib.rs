// PINK's message hover lane (transcript.rs `Transcript::render_row`'s
// metadata strip and `meta_action`): the band reserved under a settled
// transcript entry, and the timestamp and icon actions (Side chat, Fork,
// Edit, Rewind, Copy) that fade in while the entry is hovered. UI rethink
// decision 54: drawn here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies each glyph (its own icon assets,
// sized with `ACTION_ICON`), each action's wash already blended over its
// hover fade, and the timestamp's text and colour; it adds the role,
// accessible name, focus ring, tooltip, listeners, the copy/fork/edit
// behaviour and the strip's fade. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, AnyElement, Div, ElementId, Hsla, SharedString, Stateful, div, prelude::*, px,
    rems,
};

/// The reserved lane: the actions plus a lead above them, in pt at the
/// 16 px default.
pub const LANE_HEIGHT: f32 = 28.0;
/// One action's square, in pt at the 16 px default.
pub const ACTION_SIZE: f32 = 24.0;
/// An action's glyph, in pt at the 16 px default.
pub const ACTION_ICON: f32 = 14.0;
/// An action's corner radius (PINK `theme::radius::SM`).
pub const ACTION_RADIUS: f32 = 6.0;
/// The gap between the strip's parts, in fixed pixels.
pub const STRIP_GAP: f32 = 2.0;
/// The strip's lead before the timestamp, in pt at the 16 px default.
pub const STRIP_LEAD: f32 = 6.0;
/// The timestamp's text size, in pt at the 16 px default.
pub const TIMESTAMP_TEXT: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The lane under an entry: `LANE_HEIGHT` tall with the strip on its
/// bottom `ACTION_SIZE`, flush with the content column (no horizontal
/// inset). `align_end` puts the strip under a user bubble's right edge;
/// otherwise it starts on a reply's first character. The host adds the
/// strip only while the entry is hovered, so revealing it never moves the
/// layout.
pub fn lane(align_end: bool) -> Div {
    div()
        .h(ui_rems(LANE_HEIGHT))
        .pt(ui_rems(LANE_HEIGHT - ACTION_SIZE))
        .w_full()
        .flex()
        .items_center()
        .when(align_end, |lane| lane.justify_end())
}

/// The strip: a `STRIP_LEAD` lead, then the host's children (timestamp,
/// actions) in a line `STRIP_GAP` apart.
pub fn strip() -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(STRIP_GAP))
        .child(div().mr(ui_rems(STRIP_LEAD)))
}

/// The entry's timestamp in the host's quiet tone.
pub fn timestamp(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_size(ui_rems(TIMESTAMP_TEXT))
        .text_color(color)
        .child(text.into())
}

/// One action: `glyph` centred in an `ACTION_SIZE` square with the host's
/// wash (transparent at rest). The host chains its semantics, focus ring,
/// tooltip and listeners on the returned element.
pub fn action(id: impl Into<ElementId>, glyph: AnyElement, wash: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .size(ui_rems(ACTION_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(ACTION_RADIUS))
        .cursor_pointer()
        .bg(wash)
        .child(glyph)
}
