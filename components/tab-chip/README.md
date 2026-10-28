# Tab chip

`pink-tab-chip` is PINK's titlebar tab as a single-GPUI package (UI rethink
decision 54). Like the other packages under `components/` it depends on
crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/tab_chip.rs`), and
the Tab Bar's strips draw their tabs with it. PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

## What it draws

- `tab(&TabStyle)`: one chip, `TAB_HEIGHT` (28) tall at every Interface
  size, between the host's minimum and maximum widths, its parts `PART_GAP`
  apart inside `PAD_LEFT` and `PAD_RIGHT` (`PAD_RIGHT_CLOSABLE` beside a
  close slot), `TAB_RADIUS` corners. `Label::Tab` sets the 12.5 pt medium
  label every tab shares; `Label::Title` sets a lone chat's 13/18 semibold
  chrome title in the same slot. The selected fill is optional, so a resting
  tab leaves its background to the host's hover wash.
- `title(text)`: the truncating title.
- `close_slot(group, shown)`: the 20 pt close control's box, hidden until the
  pointer is over the chip's hover group (or always shown on the active tab).
- `badge(&BadgeStyle)`: the 16 pt Main badge's pill.
- `unread_dot(color)`: the 6 pt unread dot.
- `title_chevron(color)`: a title's menu chevron and related count.
- Glyph sizes (`HARNESS_MARK`, `ROLE_GLYPH`, `STATUS_GLYPH`, `CLOSE_GLYPH`,
  `BADGE_GLYPH`, `CHEVRON_GLYPH`) and `ui_rems(design)`: label text grows
  with the host's Interface size; the chip's height, padding and corners
  stay fixed so the titlebar strip never grows.

The host adds the rest: the glyphs, the spinner, the colours already
resolved for the strip's focus and the contrast floors, the id, the hover
wash and its fade, the role, accessible name and focus ring, the tooltip,
the click, keyboard, menu and drag listeners, and measurement.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-tab-chip = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-tab-chip" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/tab_chip.rs` lays tabs and a title out at Interface
  sizes 12, 16 and 20.
