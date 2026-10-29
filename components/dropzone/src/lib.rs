// PINK's attachment drop veil (shell.rs "attachment-drop-overlay"): hidden
// at rest, it covers its surface while files hover over it, a scrim with a
// centred "Drop to attach". UI rethink decision 54: drawn here, consumed by
// PINK through a pinned revision.
//
// The package draws only. The host supplies the resolved scrim and label
// colours and the label; it adds the id, the rounded corners of a card it
// covers, and the `drag_over` styles that reveal the veil (GPUI matches them
// against the dragged payload's type, so the host names the payloads it
// accepts: a resize handle or a tab drag never shows it). Plain `//`
// comments: the pink-ui gallery compiles this file in place with `include!`
// for its story.

use gpui::{AbsoluteLength, Div, Hsla, div, prelude::*, rems};

/// The scrim's strength in the dark appearance (PINK
/// `theme.scrim().opacity(0.4 / 0.6)`; the host resolves the light side).
pub const VEIL_ALPHA_DARK: f32 = 0.4;
/// The label's size, in pt at the 16 px default.
pub const LABEL_TEXT: f32 = 13.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the veil draws, already resolved by the host.
pub struct VeilStyle {
    /// The scrim over the surface while files hover.
    pub scrim: Hsla,
    /// The label's colour.
    pub label: Hsla,
}

/// The veil over its (relatively positioned) surface, transparent until a
/// `drag_over` style the host adds sets its opacity to 1. The host adds the
/// label.
pub fn veil(style: &VeilStyle) -> Div {
    div()
        .absolute()
        .inset_0()
        .opacity(0.0)
        .bg(style.scrim)
        .flex()
        .items_center()
        .justify_center()
        .text_size(ui_rems(LABEL_TEXT))
        .text_color(style.label)
}
