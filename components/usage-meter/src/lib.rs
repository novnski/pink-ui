// PINK's usage meters (shell/usage.rs `usage_meter` and `usage_meter_with`),
// the split bar that carries every part of a total in its series colour (the
// pink-ui Compact Breakdown's, adopted on PINK's Usage summary card) and the
// plan badge a usage header carries (usage.rs `compact_limit_row`). One
// meter is drawn for the sidebar card, the Usage page's limits and the
// gallery's usage cards (audit SS-04): the fill is what is used, neutral
// until `WARNING_AT`, warning from there, danger from `DANGER_AT`, on a
// capsule track, with nothing drawn below `MIN_DRAWN` so an idle window
// shows no false stub.
//
// The package draws only. The host supplies the resolved colours (the
// readable warning and danger tones, the series tones, the track) and the
// label text, and adds the ids, tooltips, hover listeners, roles and
// accessible names. The split bar and the badge size themselves in rems, so
// they follow the host's Interface size; `meter` keeps the 6 px capsule the
// compact card draws, and a host that wants it to scale sets
// `.h(ui_rems(METER_HEIGHT))`. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, SharedString, div, prelude::*, px, relative, rems};

/// A meter's capsule height, and the split bar's.
pub const METER_HEIGHT: f32 = 6.0;
/// The used share, in percent, from which the fill turns warning.
pub const WARNING_AT: f32 = 75.0;
/// The used share, in percent, from which the fill turns danger.
pub const DANGER_AT: f32 = 90.0;
/// Below this used share, in percent, the meter draws its track alone.
pub const MIN_DRAWN: f32 = 0.5;

/// The space between two parts of a split bar, in pixels.
pub const SPLIT_GAP: f32 = 2.0;
/// A part below this share of the total gets no segment of its own; the
/// drawn parts share the bar between them.
pub const SPLIT_MIN_SHARE: f64 = 0.005;
/// The split bar's row: taller than its capsule, so a segment is easy to
/// point at.
pub const SPLIT_HIT: f32 = 12.0;

/// The plan badge's padding, corner radius and type.
pub const BADGE_PAD_X: f32 = 5.0;
pub const BADGE_PAD_Y: f32 = 1.0;
pub const BADGE_RADIUS: f32 = 5.0;
pub const BADGE_TEXT: f32 = 11.0;
pub const BADGE_LINE: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a meter draws: the fill at each level and the track under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterStyle {
    pub neutral: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
    pub track: Hsla,
}

/// The fill a meter at `used_percent` draws in.
pub fn meter_fill(used_percent: f32, style: &MeterStyle) -> Hsla {
    let used = used_percent.clamp(0.0, 100.0);
    if used >= DANGER_AT {
        style.danger
    } else if used >= WARNING_AT {
        style.warning
    } else {
        style.neutral
    }
}

/// One meter: a `METER_HEIGHT` px capsule track filling its row's free
/// width, with the used share filled from the leading edge in
/// [`meter_fill`]'s tone. Nothing is filled below `MIN_DRAWN` percent.
pub fn meter(used_percent: f32, style: &MeterStyle) -> Div {
    let used = used_percent.clamp(0.0, 100.0);
    let fill = meter_fill(used, style);
    div()
        .h(px(METER_HEIGHT))
        .flex_1()
        .min_w_0()
        .rounded_full()
        .bg(style.track)
        .when(used >= MIN_DRAWN, |bar| {
            bar.child(
                div()
                    .h_full()
                    .w(relative(used / 100.0))
                    .rounded_full()
                    .bg(fill),
            )
        })
}

/// The parts a split bar draws, in the order given: `(index, share)` for
/// every part holding at least `SPLIT_MIN_SHARE` of the total, with the
/// shares of the drawn parts summing to one. Negative and non-finite values
/// count as nothing; an empty or all-zero total draws no parts.
pub fn split_shares(values: &[f64]) -> Vec<(usize, f32)> {
    let value = |v: f64| if v.is_finite() { v.max(0.0) } else { 0.0 };
    let total: f64 = values.iter().copied().map(value).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let drawn: Vec<(usize, f64)> = values
        .iter()
        .copied()
        .map(value)
        .enumerate()
        .filter(|(_, v)| v / total >= SPLIT_MIN_SHARE)
        .collect();
    let drawn_total: f64 = drawn.iter().map(|(_, v)| v).sum();
    drawn
        .into_iter()
        .map(|(index, v)| (index, (v / drawn_total) as f32))
        .collect()
}

/// The split bar's row: the full width, `SPLIT_HIT` tall, the segments in
/// a line `SPLIT_GAP` apart. Put [`split_segment`]s in it, or
/// [`split_empty`] when there is nothing to split.
pub fn split_bar() -> Div {
    div()
        .w_full()
        .h(ui_rems(SPLIT_HIT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPLIT_GAP))
}

/// One part of a split bar: `share` of the bar's width after the gaps, the
/// row's full height to point at, and the capsule piece in `tone`, rounded
/// at the bar's two ends only.
pub fn split_segment(share: f32, tone: Hsla, first: bool, last: bool) -> Div {
    let mut segment = div()
        .flex_basis(px(0.0))
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .child(
            div()
                .w_full()
                .h(ui_rems(METER_HEIGHT))
                .bg(tone)
                .when(first, |piece| piece.rounded_l(ui_rems(METER_HEIGHT / 2.0)))
                .when(last, |piece| piece.rounded_r(ui_rems(METER_HEIGHT / 2.0))),
        );
    segment.style().flex_grow = Some(share.max(0.0));
    segment
}

/// The split bar's track alone, for a total with nothing in it.
pub fn split_empty(track: Hsla) -> Div {
    div()
        .w_full()
        .h(ui_rems(METER_HEIGHT))
        .rounded_full()
        .bg(track)
}

/// What a plan badge draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadgeStyle {
    pub background: Hsla,
    pub text: Hsla,
}

/// A plan badge ("Pro", "Max"): `BADGE_TEXT`/`BADGE_LINE` on a
/// `BADGE_RADIUS` plate, padded `BADGE_PAD_X` by `BADGE_PAD_Y`, never
/// shrinking.
pub fn plan_badge(style: &BadgeStyle, label: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(BADGE_PAD_X))
        .py(px(BADGE_PAD_Y))
        .rounded(px(BADGE_RADIUS))
        .bg(style.background)
        .text_size(ui_rems(BADGE_TEXT))
        .line_height(ui_rems(BADGE_LINE))
        .text_color(style.text)
        .whitespace_nowrap()
        .child(label.into())
}
