// PINK's floating scroll rail (popover.rs `MenuScrollbarState::render_rail`
// under `rail`): the quiet thumb that appears at the right edge of a menu,
// a settings page or a panel while it scrolls, widening and brightening
// under the pointer. UI rethink decision 54: drawn here, consumed by PINK
// through a pinned revision.
//
// The package draws only. The host measures the thumb (its top inside the
// track and its height), decides when the rail shows, how far it has faded
// and whether it is active (hovered or dragged), and supplies the faint
// tone the thumb is painted from; it adds the rail's id, its hover, press,
// drag and release listeners, and the scrolling itself. Plain `//`
// comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{Div, Hsla, div, prelude::*, px};

/// The track's inset from the top and bottom of the scroll viewport; the
/// thumb travels inside it.
pub const TRACK_INSET: f32 = 4.0;
/// The invisible hit strip's width along the viewport's right edge.
pub const HIT_WIDTH: f32 = 10.0;
/// The thumb's gap from the viewport's right edge.
pub const THUMB_INSET: f32 = 2.0;
/// The resting thumb's width.
pub const THUMB_WIDTH: f32 = 3.0;
/// The thumb's width while the rail is active (hovered or dragged).
pub const ACTIVE_THUMB_WIDTH: f32 = 5.0;
/// The shortest thumb, so a very long list still has one to grab.
pub const MIN_THUMB: f32 = 24.0;

/// The resting thumb's alpha over the faint tone.
pub const THUMB_ALPHA: f32 = 0.5;
/// The thumb's alpha while the rail is active.
pub const ACTIVE_THUMB_ALPHA: f32 = 0.68;
/// The thumb's alpha under the pointer.
pub const HOVER_THUMB_ALPHA: f32 = 0.85;

/// The thumb's width, wider while the rail is `active`.
pub fn thumb_width(active: bool) -> f32 {
    if active {
        ACTIVE_THUMB_WIDTH
    } else {
        THUMB_WIDTH
    }
}

/// The thumb's paint from the host's `faint` tone, brighter while the rail
/// is `active`.
pub fn thumb_color(faint: Hsla, active: bool) -> Hsla {
    faint.opacity(if active {
        ACTIVE_THUMB_ALPHA
    } else {
        THUMB_ALPHA
    })
}

/// The rail: a full-height `HIT_WIDTH` strip absolutely positioned on its
/// parent's right edge at `fade` opacity, holding the thumb `thumb_top`
/// below the track's top (`TRACK_INSET` down) and `thumb_height` tall,
/// `THUMB_INSET` from the edge, a capsule of `thumb_width(active)` in
/// `thumb_color(faint, active)` that brightens to `HOVER_THUMB_ALPHA` under
/// the pointer. The thumb sits inside a fixed strip, so widening it never
/// reflows the rows. The host chains the strip's id first, then its
/// listeners. The thumb's debug selector is `scrollbar-thumb`.
pub fn rail(faint: Hsla, fade: f32, active: bool, thumb_top: f32, thumb_height: f32) -> Div {
    let width = thumb_width(active);
    div()
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .right(px(0.0))
        .w(px(HIT_WIDTH))
        .opacity(fade)
        .child(
            div()
                .id("scrollbar-thumb")
                .debug_selector(|| "scrollbar-thumb".into())
                .absolute()
                .top(px(TRACK_INSET + thumb_top))
                .right(px(THUMB_INSET))
                .w(px(width))
                .h(px(thumb_height))
                .rounded(px(width / 2.0))
                .bg(thumb_color(faint, active))
                .hover(|style| style.bg(faint.opacity(HOVER_THUMB_ALPHA))),
        )
}
