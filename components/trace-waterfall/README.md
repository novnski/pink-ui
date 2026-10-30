# Trace waterfall

`pink-trace-waterfall` is the time side of the pink-ui Trace Waterfall as a
single-GPUI package (novnski/pink#266): a run's calls on one shared time
axis. Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this same
`src/lib.rs` in place (`src/components/trace_waterfall.rs`); the Trace
Waterfall story's axis, grid, duration bars and labels draw with it. PINK pins
the package and patches `gpui` to its vendored runtime, so there is one GPUI
type graph and no copied source.

## What it draws

- `TraceAxis`: seconds from the run's start to a fraction of the plot.
  `TraceAxis::finished(total)` gives a finished run 2% of room after its end;
  `ticks()` starts at zero and keeps at most `MAX_TICKS` (6) readable steps
  (`tick_step`).
- `axis_ticks(&axis, text)`: the axis header's timeline cell, each tick's
  time (`fmt_tick`) just after its line. The host draws the step column's
  header beside it.
- `grid(&axis, hairline)`: a 1 px hairline at every tick but zero, filling
  the timeline column's box behind the bars.
- `bar_cell(&axis, start, end, label, &BarStyle)`: one call's cell, its
  `BAR_H` (6 pt) capsule from `start` to `end` and its label `LABEL_GAP`
  after it. A bar is never shorter than `BAR_H`, so a call with no recorded
  end draws only that mark at its start (never a bar to the end of the run);
  a call with no recorded start draws no bar and its label sits at zero.
- `fmt_secs`, `fmt_cost` and `bar_label(secs, cost)`: Pink's timer and live
  money figures (`12.3s`, `1m 05s`, `$0.031`). An explicit zero cost reads
  `$0.000`; a `None` cost stays out of the label, so saying it is unknown is
  the host's call.
- `ui_rems(design)`: every size is rems (pt at the 16 px default), so the
  waterfall follows the host's Interface size; axis positions are fractions
  of the plot (`PAD_L` before zero, `PAD_R` kept after the end for the last
  label), so it fits any column width.

The host adds the rest: the resolved colours (each call's bar tone, the label
and tick text, the hairline), every label's text, the step column beside the
bars, and the ids, roles, accessible names, focus and listeners.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-trace-waterfall = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-trace-waterfall" }
```

`transcript/trace.rs` lays a finished turn's recorded tool and model calls
(the engine's `callSpans` index) out on the host's observed clock, with each
call's recorded duration and, when the provider reported one, its cost.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/trace_waterfall.rs` lays a closed, an open and an
  untimed call out at Interface sizes 12, 16 and 20 and checks the ticks and
  figures.
