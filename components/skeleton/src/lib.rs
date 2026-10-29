// PINK's pulsing skeletons (popover.rs `skeleton_rows`, `skeleton_menu_rows`
// and `skeleton_bar`): the ghost rows a list, a settings page or a menu shows
// while it loads, and the ghost label a trigger chip holds while its value
// resolves. UI rethink decision 54: drawn here, consumed by PINK through a
// pinned revision.
//
// The package draws only. The host supplies the wash, its pulse clock's
// phase (0..1, the same for every skeleton on screen, resting at 0 under
// reduced motion) and the wave that turns a phase into brightness; it
// schedules the repaints. Plain `//` comments: the pink-ui gallery compiles
// this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, div, prelude::*, px, relative, rems};

/// How far each row trails the one above it in the pulse, as a share of
/// the period, so the pulse ripples down the list.
pub const STAGGER: f32 = 0.08;
/// A skeleton's opacity at the trough of its pulse.
pub const OPACITY_REST: f32 = 0.35;
/// How much brighter a skeleton gets at the crest of its pulse.
pub const OPACITY_SWING: f32 = 0.4;

/// A list row's height, in pt at the 16 px default.
pub const ROW_HEIGHT: f32 = 28.0;
/// The gap between list rows, in pt at the 16 px default.
pub const ROW_GAP: f32 = 6.0;
/// The padding above and below the list rows, in pt at the 16 px default.
pub const ROWS_PAD_Y: f32 = 4.0;
/// A list row's corner radius: the host's control radius.
pub const ROW_RADIUS: f32 = 6.0;

/// A menu row's height, in pt at the 16 px default.
pub const MENU_ROW_HEIGHT: f32 = 14.0;
/// The gap between menu rows, in pt at the 16 px default.
pub const MENU_ROW_GAP: f32 = 8.0;
/// The padding above and below the menu rows, in pt at the 16 px default.
pub const MENU_PAD_Y: f32 = 6.0;
/// The padding beside the menu rows, in pt at the 16 px default.
pub const MENU_PAD_X: f32 = 4.0;
/// A menu row's corner radius: a pill.
pub const MENU_ROW_RADIUS: f32 = 7.0;
/// The menu rows' widths as shares of the menu, cycled from the top: a
/// fixed ladder, so the rows read as ghost labels of varied length and
/// repaint the same on every open.
pub const MENU_ROW_WIDTHS: [f32; 4] = [0.42, 0.58, 0.48, 0.66];

/// A ghost label's height, in pt at the 16 px default.
pub const BAR_HEIGHT: f32 = 11.0;
/// A ghost label's corner radius: a pill.
pub const BAR_RADIUS: f32 = 5.5;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// Row `index`'s pulse phase: the clock's `phase` less `index` times
/// `STAGGER`, wrapped into 0..1.
pub fn row_phase(phase: f32, index: usize) -> f32 {
    (phase - index as f32 * STAGGER).rem_euclid(1.0)
}

/// A skeleton's opacity for a wave value (0 at the trough, 1 at the crest).
pub fn opacity(wave: f32) -> f32 {
    OPACITY_REST + OPACITY_SWING * wave
}

/// Pulsing list rows: `count` full-width bars `ROW_HEIGHT` tall, rounded to
/// `ROW_RADIUS`, `ROW_GAP` apart with `ROWS_PAD_Y` above and below, in
/// `wash`. Row `i` pulses at `opacity(wave(row_phase(phase, i)))`.
pub fn rows(count: usize, wash: Hsla, phase: f32, wave: impl Fn(f32) -> f32) -> Div {
    let wave = &wave;
    div()
        .flex()
        .flex_col()
        .gap(ui_rems(ROW_GAP))
        .py(ui_rems(ROWS_PAD_Y))
        .children((0..count).map(move |i| {
            div()
                .h(ui_rems(ROW_HEIGHT))
                .rounded(px(ROW_RADIUS))
                .bg(wash)
                .opacity(opacity(wave(row_phase(phase, i))))
        }))
}

/// Pulsing rows shaped like a loading menu: `count` pill bars
/// `MENU_ROW_HEIGHT` tall at the `MENU_ROW_WIDTHS` shares, `MENU_ROW_GAP`
/// apart, inset `MENU_PAD_X` by `MENU_PAD_Y`, in `wash`, staggered like
/// `rows`.
pub fn menu_rows(count: usize, wash: Hsla, phase: f32, wave: impl Fn(f32) -> f32) -> Div {
    let wave = &wave;
    div()
        .flex()
        .flex_col()
        .gap(ui_rems(MENU_ROW_GAP))
        .py(ui_rems(MENU_PAD_Y))
        .px(ui_rems(MENU_PAD_X))
        .children((0..count).map(move |i| {
            div()
                .h(ui_rems(MENU_ROW_HEIGHT))
                .w(relative(MENU_ROW_WIDTHS[i % MENU_ROW_WIDTHS.len()]))
                .rounded(px(MENU_ROW_RADIUS))
                .bg(wash)
                .opacity(opacity(wave(row_phase(phase, i))))
        }))
}

/// One pulsing ghost label in a label's slot: `width` (pt at the 16 px
/// default) by `BAR_HEIGHT`, a pill in `wash`, pulsing at
/// `opacity(wave(row_phase(phase, 0)))`.
pub fn bar(width: f32, wash: Hsla, phase: f32, wave: impl Fn(f32) -> f32) -> Div {
    div()
        .w(ui_rems(width))
        .h(ui_rems(BAR_HEIGHT))
        .rounded(px(BAR_RADIUS))
        .bg(wash)
        .opacity(opacity(wave(row_phase(phase, 0))))
}
