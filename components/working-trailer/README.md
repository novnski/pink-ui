# Working trailer

`pink-working-trailer` is PINK's live line under the last transcript row as
a single-GPUI package (UI rethink decision 54): the breathing orb, the
status, the elapsed timer and the quiet Stop that `render_working_trailer`
draws while a turn runs. Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/working_trailer.rs`); its loaders' orb, status and timer
and the Thinking Loader's trailer draw through it. PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

## What it draws

- `trailer(id)`: the row, `LEAD` above and items `GAP` apart (both fixed
  pixels), top-aligned so the orb, timer and Stop stay on the first line of
  a wrapped sentence. It names the `GROUP` the Stop reveals on.
- `orb(accent, breath)` and `orb_slot(accent, breath)`: the `ORB_BOX` orb,
  its core growing from 85% to `ORB_CORE` and its halo from 16% to 34% of
  `accent` as `breath` runs 0 to 1, centred on one `LINE_HEIGHT` line box.
- `status_line(id)`, `verb(text, tone, sentence)`, `shimmered(verb,
  overlay)` and `object(text, color)`: the `STATUS_TEXT` status, the verb
  and its truncating object `STATUS_GAP` apart. A status word stays on one
  line under the host's shimmer; a sentence wraps.
- `timer(text, color)`: the `TIMER_TEXT` timer, never wrapping.
- `stop(id, label, color, wash)`: the Stop, `STOP_LEAD` after the timer,
  `STOP_PAD_X` padding, `STOP_RADIUS` corners, transparent until the pointer
  is over the trailer.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. The
  text, line box, status gap and the Stop's lead and padding grow with the
  host's Interface size; the lead, gap, orb and corners stay fixed.

The host adds the rest: the words (including the timer's text), the status
role and accessible name, the shimmer overlay and its phase, the orb's
breath and reduced motion, the status swap animation, tabular figures on
the timer, the Stop's hover fade, focus reveal, focus ring and listeners,
and every decision about which state to show.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-working-trailer = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-working-trailer" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/working_trailer.rs` lays a one-word status and a
  wrapped sentence out at Interface sizes 12, 16 and 20.
