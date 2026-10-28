// PINK's feedback toast (shell.rs `Shell::render_toast`): a capsule
// centered just above the composer with the notice's tone glyph, one
// truncating line of copy and a round Dismiss, arriving with a short fade and
// rise. UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the surface, edge and text
// colours, its shadows and font, the glyphs and the copy; it adds the ids,
// the status role and accessible names, the focus ring, the hover wash, the
// dismiss listener, the animation timing and the notice's lifetime. Plain
// `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{AbsoluteLength, BoxShadow, Div, Hsla, SharedString, div, prelude::*, px, rems};

/// The capsule's height. Fixed: the toast does not grow with the Interface
/// size (only its copy does).
pub const TOAST_HEIGHT: f32 = 34.0;
/// The space between the capsule and its anchor's top edge.
pub const ANCHOR_GAP: f32 = 8.0;
/// The widest the capsule grows before its message truncates.
pub const MAX_WIDTH: f32 = 520.0;
/// The leading padding, and the trailing one beside Dismiss (the control's
/// own size carries the rest).
pub const PAD_LEFT: f32 = 12.0;
pub const PAD_RIGHT: f32 = 4.0;
/// The gap between the capsule's parts.
pub const GAP: f32 = 8.0;
/// The message, in pt at the 16 px default.
pub const TEXT: f32 = 12.5;
/// The tone glyph.
pub const GLYPH: f32 = 14.0;
/// The round Dismiss control and its close glyph.
pub const DISMISS_SIZE: f32 = 24.0;
pub const DISMISS_GLYPH: f32 = 10.0;
/// How far the capsule rises while it arrives.
pub const RISE: f32 = 8.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the capsule draws, already resolved by the host.
pub struct ToastStyle {
    /// The opaque surface the capsule sits on.
    pub surface: Hsla,
    /// The hairline edge.
    pub edge: Hsla,
    /// The message's colour.
    pub text: Hsla,
    pub shadows: Vec<BoxShadow>,
    pub font: SharedString,
}

/// The lane that floats the toast: absolutely placed `TOAST_HEIGHT +
/// ANCHOR_GAP` above its `.relative()` anchor's top edge, centering the
/// capsule across it.
pub fn lane() -> Div {
    div()
        .absolute()
        .top(px(-(TOAST_HEIGHT + ANCHOR_GAP)))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
}

/// The capsule: the parts in a line, fully rounded on the host's surface.
/// The host adds the id, the status role and name, the glyph (`GLYPH` px),
/// the [`message`] and the [`dismiss`] control.
pub fn capsule(style: &ToastStyle) -> Div {
    div()
        .h(px(TOAST_HEIGHT))
        .max_w(px(MAX_WIDTH))
        .pl(px(PAD_LEFT))
        .pr(px(PAD_RIGHT))
        .rounded_full()
        .border_1()
        .border_color(style.edge)
        .bg(style.surface)
        .shadow(style.shadows.clone())
        .flex()
        .items_center()
        .gap(px(GAP))
        .font_family(style.font.clone())
        .text_size(ui_rems(TEXT))
        .text_color(style.text)
}

/// The one line of copy: it takes the room that is left and truncates.
pub fn message(text: impl Into<SharedString>) -> Div {
    div().min_w_0().truncate().child(text.into())
}

/// The round Dismiss control's frame. The host adds the id, the role and
/// name, the tab stop, the hover wash, the focus ring, the click and the
/// close glyph (`DISMISS_GLYPH` px).
pub fn dismiss() -> Div {
    div()
        .size(px(DISMISS_SIZE))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
}

/// The arrival at `progress` (0 to 1) of the host's animation: a fade and a
/// `RISE` into place. Apply it to a `.relative()` wrapper of the capsule.
pub fn arrive(wrapper: Div, progress: f32) -> Div {
    wrapper.opacity(progress).top(px(RISE * (1.0 - progress)))
}
