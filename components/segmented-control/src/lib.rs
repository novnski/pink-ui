// PINK's segmented control (button.rs `segmented`): equal-width options on
// a quiet track with a hairline edge, and one thumb that slides to the
// chosen option instead of each option swapping its own fill. The thumb is
// placed by a fraction of the track's content box, so nothing is measured,
// and the host animates its position. In the light appearance the thumb
// lifts off the track with a hairline edge and a soft shadow.
//
// The track's width, height and type are design sizes in pt at the 16 px
// default and become rems (`ui_rems`), so labels keep their room at every
// Interface size. The inset around the thumb and the corners stay in px,
// as PINK draws them.
//
// The package draws only. The host supplies the text, the glyph elements
// and the colours, already resolved and blended over its hover fade, and
// adds what crates.io GPUI 0.2.2 can't express (roles, accessible names,
// focus and its ring) and the behaviour (click, keyboard, animation).

use gpui::{
    AnyElement, BoxShadow, Div, ElementId, FontWeight, Hsla, Rems, SharedString, Stateful, div,
    prelude::*, px, relative, rems,
};

/// A design size (pt at the 16 px default) as rems, the same formula as the
/// host's `ui_rems`.
pub fn ui_rems(design: f32) -> Rems {
    rems(design / 16.0)
}

/// The track's inset around the thumb, in px.
pub const INSET: f32 = 2.0;
/// An option's horizontal padding.
pub const OPTION_PAD_X: f32 = 6.0;
/// The gap between an option's glyph and its label.
pub const OPTION_GAP: f32 = 5.0;
/// The smallest corner the thumb keeps (PINK's `radius::XS`).
pub const MIN_THUMB_RADIUS: f32 = 4.0;

/// The thumb's corner inside a track with `radius` corners (PINK's
/// `radius::inner`): concentric with the track, never sharper than
/// `MIN_THUMB_RADIUS`.
pub fn thumb_radius(radius: f32) -> f32 {
    (radius - INSET).max(MIN_THUMB_RADIUS)
}

/// The track's measure and paint, from the host's control size and theme.
pub struct Track {
    /// The whole control's width in design pt, or `None` to fill the
    /// container.
    pub width: Option<f32>,
    /// The control's height in design pt.
    pub height: f32,
    /// The track's corner, in px.
    pub radius: f32,
    /// The labels' type size in design pt.
    pub text: f32,
    pub fill: Hsla,
    pub edge: Hsla,
}

/// The light appearance's lift: the thumb's hairline edge and its soft
/// shadow (PINK: 8% black, 1 px down, 2 px blur), built by the host.
pub struct Lift {
    pub edge: Hsla,
    pub shadows: Vec<BoxShadow>,
}

/// The thumb under the chosen option. `position` is its option index, a
/// fraction while it slides, out of `count` options.
pub fn thumb(position: f32, count: usize, radius: f32, fill: Hsla, lift: Option<Lift>) -> Div {
    let count = count.max(1) as f32;
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(position / count))
        .w(relative(1.0 / count))
        .rounded(px(thumb_radius(radius)))
        .bg(fill)
        .when_some(lift, |el, lift| {
            el.border_1().border_color(lift.edge).shadow(lift.shadows)
        })
}

/// One option's box: an equal share of the track, its content centred, the
/// chosen one in medium weight. The host adds the role, name, state, focus,
/// listeners and its text colour, then the glyph and `label`.
pub fn option(id: impl Into<ElementId>, chosen: bool, radius: f32) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex_1()
        .min_w_0()
        .h_full()
        .px(ui_rems(OPTION_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(ui_rems(OPTION_GAP))
        .rounded(px(thumb_radius(radius)))
        .cursor_pointer()
        .font_weight(if chosen {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        })
}

/// An option's label, truncated when the share is too narrow.
pub fn label(text: impl Into<SharedString>) -> Div {
    div().min_w_0().truncate().child(text.into())
}

/// The control: the track, the thumb (none while nothing is chosen) and
/// the options in a row over it. The host adds the control's id and role.
pub fn tray(track: &Track, thumb: Option<Div>, options: Vec<AnyElement>) -> Div {
    div()
        .flex_none()
        .map(|el| match track.width {
            Some(width) => el.w(ui_rems(width)),
            None => el.w_full(),
        })
        .h(ui_rems(track.height))
        .p(px(INSET))
        .rounded(px(track.radius))
        .bg(track.fill)
        .border_1()
        .border_color(track.edge)
        .text_size(ui_rems(track.text))
        .child(
            // The content box the thumb's fractions are taken against.
            div().relative().size_full().children(thumb).child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .flex_row()
                    .children(options),
            ),
        )
}
