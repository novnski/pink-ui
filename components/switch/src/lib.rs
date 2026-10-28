// PINK's toggle switch (settings/widgets.rs `toggle_switch`): a macOS-style
// pill whose knob slides right and whose track fills when on. UI rethink
// decision 54: drawn here, consumed by PINK through a pinned revision.
//
// The package draws only. The host owns the motion: it eases `progress`
// from 0 (off) to 1 (on) when the value flips and passes the track colour
// already mixed between its off and on tones at that progress. It adds the
// id, the role and toggled state, the focus ring, the cursor and the click.
// Sizes are rems, so the switch follows the host's Interface size. Plain
// `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, div, prelude::*, rems};

/// The track's width.
pub const TRACK_W: f32 = 34.0;
/// The track's height.
pub const TRACK_H: f32 = 20.0;
/// The knob's side.
pub const KNOB: f32 = 16.0;
/// The knob's inset from the track's ends.
pub const KNOB_INSET: f32 = 2.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the switch draws.
pub struct SwitchStyle {
    /// The track, already mixed between its off and on tones at `progress`.
    pub track: Hsla,
    pub knob: Hsla,
}

/// The knob's left edge at `progress` (0 off, 1 on), in pt at the 16 px
/// default.
pub fn knob_left(progress: f32) -> f32 {
    let from = KNOB_INSET;
    let to = TRACK_W - KNOB - KNOB_INSET;
    from + (to - from) * progress
}

/// The track: a `TRACK_W` by `TRACK_H` pill that positions its knob.
pub fn track(style: &SwitchStyle) -> Div {
    div()
        .flex_none()
        .w(ui_rems(TRACK_W))
        .h(ui_rems(TRACK_H))
        .rounded_full()
        .bg(style.track)
        .relative()
}

/// The round `KNOB`, vertically centred in the track and lifted by a small
/// shadow, at `progress` along its travel.
pub fn knob(style: &SwitchStyle, progress: f32) -> Div {
    div()
        .absolute()
        .top(ui_rems((TRACK_H - KNOB) / 2.0))
        .left(ui_rems(knob_left(progress)))
        .size(ui_rems(KNOB))
        .rounded_full()
        .bg(style.knob)
        .shadow_sm()
}

/// The switch: the `track` holding its `knob` at `progress`.
pub fn switch(style: &SwitchStyle, progress: f32) -> Div {
    track(style).child(knob(style, progress))
}
