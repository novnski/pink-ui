// PINK's working trailer (transcript.rs `Transcript::render_working_trailer`
// and loaders.rs `breathing_orb`): the bare row under the last transcript
// row while a turn runs. The breathing accent orb on a one-line box, the
// status (a verb, then its object), the elapsed timer and a "Stop" that only
// shows under the pointer. UI rethink decision 54: drawn here, consumed by
// PINK through a pinned revision.
//
// The package draws only. The host supplies the words (the verb, object,
// timer text and Stop label), each colour already resolved, the orb's breath
// and the Stop's wash already blended over its hover fade; it adds the
// status role and accessible name, the shimmer overlay, the status swap
// animation, tabular figures on the timer, the Stop's focus reveal, focus
// ring and listeners. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, AnyElement, Div, ElementId, Hsla, SharedString, Stateful, div, prelude::*, px,
    rems,
};

/// The hover group the Stop listens to.
pub const GROUP: &str = "working-trailer";
/// The lead above the trailer, in fixed pixels (PINK `Theme::SPACE_LG`).
pub const LEAD: f32 = 16.0;
/// The gap between the orb, status, timer and Stop, in fixed pixels (PINK
/// `Theme::SPACE_SM`).
pub const GAP: f32 = 8.0;
/// The status text size, in pt at the 16 px default.
pub const STATUS_TEXT: f32 = 12.0;
/// The shared line box every item sits on, in pt at the 16 px default.
pub const LINE_HEIGHT: f32 = 16.0;
/// The gap between the verb and its object, in pt at the 16 px default.
pub const STATUS_GAP: f32 = 4.0;
/// The timer and Stop text size, one step quieter, in pt at the 16 px
/// default.
pub const TIMER_TEXT: f32 = 11.0;
/// The orb's box, in fixed pixels.
pub const ORB_BOX: f32 = 12.0;
/// The orb's solid core at full breath, in fixed pixels.
pub const ORB_CORE: f32 = 6.0;
/// The Stop's extra lead after the timer, in pt at the 16 px default.
pub const STOP_LEAD: f32 = 4.0;
/// The Stop's horizontal padding, in pt at the 16 px default.
pub const STOP_PAD_X: f32 = 6.0;
/// The Stop's corner radius, in fixed pixels (PINK `Theme::CONTROL_RADIUS`).
pub const STOP_RADIUS: f32 = 6.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The trailer row: `LEAD` above, items `GAP` apart. Every item is one line
/// box tall and top-aligned, so the orb, timer and Stop stay on the first
/// line of a wrapped sentence. It names the `GROUP` the Stop reveals on.
pub fn trailer(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .group(GROUP)
        .flex()
        .flex_row()
        .items_start()
        .gap(px(GAP))
        .pt(px(LEAD))
        .min_w_0()
}

/// The breathing orb: a solid `accent` core inside a soft halo. `breath`
/// runs 0 (rest) to 1 (full): the core grows from 85% to `ORB_CORE`, the
/// halo from 16% to 34% `accent`. The host drives the breath (and rests it
/// at 1 under reduced motion).
pub fn orb(accent: Hsla, breath: f32) -> Div {
    let core = ORB_CORE * (0.85 + 0.15 * breath);
    div()
        .flex_none()
        .relative()
        .size(px(ORB_BOX))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .bg(accent.opacity(0.16 + 0.18 * breath)),
        )
        .child(div().size(px(core)).rounded_full().bg(accent))
}

/// The orb's slot: one `LINE_HEIGHT` line box centring the orb.
pub fn orb_slot(accent: Hsla, breath: f32) -> Div {
    div()
        .flex_none()
        .h(ui_rems(LINE_HEIGHT))
        .flex()
        .items_center()
        .child(orb(accent, breath))
}

/// The status line: `STATUS_TEXT` on the shared line box, the verb and its
/// object `STATUS_GAP` apart. It may shrink, so a sentence wraps in the
/// width the timer leaves. The host adds the role, accessible name and the
/// swap animation, then the verb and object.
pub fn status_line(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(STATUS_GAP))
        .text_size(ui_rems(STATUS_TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
}

/// The verb in its `tone`. A status word stays on one line (the host paints
/// its shimmer over it, see `shimmered`); a `sentence` (a queued line or a
/// provider notice) may shrink and wrap.
pub fn verb(text: impl Into<SharedString>, tone: Hsla, sentence: bool) -> Div {
    let verb = div().text_color(tone).child(text.into());
    if sentence {
        verb.min_w_0()
    } else {
        verb.flex_none().whitespace_nowrap()
    }
}

/// A status word under the host's shimmer overlay (painted over the same
/// text at the same size and line height).
pub fn shimmered(verb: Div, overlay: AnyElement) -> Div {
    div().relative().flex_none().child(verb).child(overlay)
}

/// What the verb acts on (a file, command or host), cut to one line.
pub fn object(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_color(color)
        .child(text.into())
}

/// The elapsed timer at `TIMER_TEXT` on the shared line box, never wrapping.
/// The host adds tabular figures so ticking digits never shift the line.
pub fn timer(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_size(ui_rems(TIMER_TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(color)
        .child(text.into())
}

/// The quiet Stop: `label` at `TIMER_TEXT`, transparent until the pointer
/// is over the trailer's `GROUP`, on the host's `wash`. The host adds the
/// button role, accessible name, focus reveal and ring, and the listeners.
pub fn stop(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    color: Hsla,
    wash: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .ml(ui_rems(STOP_LEAD))
        .px(ui_rems(STOP_PAD_X))
        .rounded(px(STOP_RADIUS))
        .cursor_pointer()
        .text_size(ui_rems(TIMER_TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(color)
        .opacity(0.0)
        .group_hover(GROUP, |style| style.opacity(1.0))
        .bg(wash)
        .child(label.into())
}
