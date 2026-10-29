# Usage meter

`pink-usage-meter` is PINK's usage meter, its provider split bar and its plan
badge as a single-GPUI package (novnski/pink#259). Like the other packages
under `components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/usage_meter.rs`); the Usage Meter story's meters and plan
badge and the Compact Breakdown story's split bar draw with it. PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI type
graph and no copied source.

## What it draws

- `meter(used_percent, &MeterStyle)`: one `METER_HEIGHT` px capsule filling
  its row's free width. The fill is what is used: `neutral` below
  `WARNING_AT` (75%), `warning` from there, `danger` from `DANGER_AT` (90%),
  and nothing below `MIN_DRAWN` (0.5%), so an idle window shows no stub.
  `meter_fill` names the tone alone. A host that wants the meter to scale
  with its text sets `.h(ui_rems(METER_HEIGHT))`, as PINK's Usage page does.
- `split_shares(values)`: which parts a split bar draws and their share of
  it. Parts under `SPLIT_MIN_SHARE` (0.5%) of the total get no segment and
  the drawn parts share the bar; negative and non-finite values count as
  nothing; an empty or zero total draws nothing.
- `split_bar()`, `split_segment(share, tone, first, last)` and
  `split_empty(track)`: the row (`SPLIT_HIT` tall, the full width), one part
  each in its series tone, `SPLIT_GAP` px apart and rounded only at the
  bar's two ends, and the bare track for a total with nothing in it.
- `plan_badge(&BadgeStyle, label)`: the subscription name ("Pro") on a
  small raised plate beside a provider's name.
- `ui_rems(design)`: the split bar and the badge's type are rems (pt at the
  16 px default), so they follow the host's Interface size; the gap, the
  badge padding and corners stay in pixels.

The host adds the rest: the resolved colours (the readable warning and
danger tones, the series tones, the track), the ids, tooltips and hover
listeners (PINK reads a hovered part out beside the total), and the roles
and accessible names.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-usage-meter = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-usage-meter" }
```

`shell/usage.rs` maps its theme onto `MeterStyle` and `BadgeStyle` for the
sidebar's compact Usage card and the Usage page, and draws the summary
card's provider split bar from the engine's usage buckets.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/usage_meter.rs` lays the split bar, the meter and
  the badge out at Interface sizes 12, 16 and 20 and checks the meter's
  thresholds and the split shares.
