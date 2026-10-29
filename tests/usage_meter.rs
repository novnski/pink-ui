//! The `pink-usage-meter` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its Usage page out. Failure modes: a split bar whose
//! segments plus gaps overflow or fall short of its row, or whose widths
//! stop following the parts' shares; a part too small to see that still
//! draws a sliver, or a dropped part whose share is lost instead of shared
//! between the drawn ones; an all-zero or empty total that draws segments
//! (or NaN widths) instead of the bare track; a split bar or plan badge
//! that stays at its 16 px size while the text beside it grows; a meter
//! whose fill stops at the wrong share, turns warning or danger at the
//! wrong threshold, or draws a stub for an idle window.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::usage_meter::{
        meter_style,
        package::{
            BADGE_LINE, BADGE_PAD_Y, BadgeStyle, METER_HEIGHT, SPLIT_GAP, SPLIT_HIT, meter,
            meter_fill, plan_badge, split_bar, split_empty, split_segment, split_shares,
        },
    },
    pink::Pink,
};

/// Claude, Codex and a part far under 0.5% of the total.
const VALUES: [f64; 3] = [62.0, 38.0, 0.2];
const WIDTH: f32 = 480.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = meter_style(&t);
        let shares = split_shares(&VALUES);
        let n = shares.len();
        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .child(split_bar().debug_selector(|| "bar".into()).children(
                shares.iter().enumerate().map(|(pos, (index, share))| {
                    split_segment(*share, t.text, pos == 0, pos + 1 == n)
                        .debug_selector(move || format!("part-{index}"))
                }),
            ))
            .child(
                split_bar()
                    .debug_selector(|| "empty-bar".into())
                    .when(split_shares(&[0.0, 0.0]).is_empty(), |bar| {
                        bar.child(split_empty(style.track).debug_selector(|| "track".into()))
                    }),
            )
            .child(
                div()
                    .flex()
                    .child(meter(40.0, &style).debug_selector(|| "meter".into())),
            )
            .child(
                div().flex().child(
                    plan_badge(
                        &BadgeStyle {
                            background: t.surface_raised,
                            text: t.text_muted,
                        },
                        "Pro",
                    )
                    .debug_selector(|| "badge".into()),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} was not drawn"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.75
}

#[gpui::test]
fn split_bar_and_badge_follow_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // The row is SPLIT_HIT tall in rems and spans its container.
        let bar = bounds(cx, "bar");
        assert!(
            near(f32::from(bar.size.height), SPLIT_HIT * scale)
                && near(f32::from(bar.size.width), WIDTH),
            "split row at {ui}: {:?}",
            bar.size
        );

        // The tiny part draws nothing; the two drawn parts plus one gap
        // fill the row exactly, in proportion to their shares of what is
        // drawn.
        assert!(cx.debug_bounds("part-2").is_none(), "sliver at {ui}");
        let claude = bounds(cx, "part-0");
        let codex = bounds(cx, "part-1");
        let avail = WIDTH - SPLIT_GAP;
        let drawn = (VALUES[0] + VALUES[1]) as f32;
        assert!(
            near(
                f32::from(claude.size.width),
                avail * VALUES[0] as f32 / drawn
            ) && near(
                f32::from(codex.size.width),
                avail * VALUES[1] as f32 / drawn
            ),
            "segment widths at {ui}: {:?} {:?}",
            claude.size,
            codex.size
        );
        assert!(
            near(f32::from(codex.origin.x - claude.right()), SPLIT_GAP)
                && near(f32::from(codex.right()), f32::from(bar.right())),
            "segments must tile the row at {ui}"
        );

        // Nothing to split: the bare track, full width, at the meter's
        // scaled height.
        let track = bounds(cx, "track");
        assert!(
            near(f32::from(track.size.width), WIDTH)
                && near(f32::from(track.size.height), METER_HEIGHT * scale),
            "empty track at {ui}: {:?}",
            track.size
        );

        // The compact meter stays a 6 px capsule; the badge's text line
        // grows with the Interface size while its pixel padding does not.
        let meter = bounds(cx, "meter");
        assert!(
            near(f32::from(meter.size.height), METER_HEIGHT),
            "meter height at {ui}: {:?}",
            meter.size
        );
        let badge = bounds(cx, "badge");
        assert!(
            near(
                f32::from(badge.size.height),
                BADGE_LINE * scale + 2.0 * BADGE_PAD_Y
            ),
            "badge height at {ui}: {:?}",
            badge.size
        );
    }
}

#[gpui::test]
fn meter_levels_and_split_shares(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let t = cx.update(|cx| Pink::get(cx));
    let style = meter_style(&t);
    // Thresholds: neutral below 75%, warning from 75%, danger from 90%;
    // out-of-range values clamp.
    for (used, expected) in [
        (0.0, style.neutral),
        (74.9, style.neutral),
        (75.0, style.warning),
        (89.9, style.warning),
        (90.0, style.danger),
        (140.0, style.danger),
        (-5.0, style.neutral),
    ] {
        assert_eq!(meter_fill(used, &style), expected, "fill at {used}%");
    }
    // Shares: order kept, slivers dropped and their share redistributed,
    // nonsense values counted as nothing, nothing drawn for no total.
    let shares = split_shares(&[3.0, f64::NAN, 1.0, -4.0, 0.0001]);
    assert_eq!(
        shares.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert!((shares[0].1 - 0.75).abs() < 1e-6 && (shares[1].1 - 0.25).abs() < 1e-6);
    assert!(split_shares(&[]).is_empty());
    assert!(split_shares(&[0.0, -1.0, f64::INFINITY]).is_empty());
}
