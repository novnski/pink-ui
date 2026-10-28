# Settings section

`pink-settings-section` is PINK's settings section anatomy as a single-GPUI
package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/settings_section.rs`), and the kit's `pink::section_card`,
`section_header`, `card_row`, `row_text`, `row_title`, `row_description`,
`row_controls`, `row_block` and `section_footer` draw through it. PINK pins
the package and patches `gpui` to its vendored runtime, so there is one GPUI
type graph and no copied source.

## What it draws

- `section_card(&CardStyle)`: one rounded group per section, `SECTION_GAP`
  above it, a 1 px border, `CARD_RADIUS` corners, clipped, a column.
- `section_header()` with `header_text()` and `header_title(color, title)`:
  the section's title block inside its card (15/20 semibold), its
  description and trailing actions on the title's line, wrapping.
- `card_row(divider)`: one row, at least `ROW_MIN_HEIGHT` tall, padded
  `ROW_PAD_X` by `ROW_PAD_Y`, a 1 px hairline on top unless it is the first,
  wrapping so its controls drop under the text when they no longer fit.
- `row_text()`, `row_title(color, title)` (13/18 medium),
  `row_description(color, copy)` (12/16): the text column, holding
  `ROW_TEXT_MIN` before the controls wrap.
- `row_controls()` and `row_block()`: the trailing control group (one line,
  pushed right) and the full-width block under the text.
- `section_footer(color, copy)`: fine print under a card on its content edge.
- `ui_rems(design)`: every size next to text is a rem (pt at the 16 px
  default), so a page follows the host's Interface size; hairlines and
  corners stay in pixels.

The host adds the rest: the resolved colours (the card tone thinned over
glass, the readable secondary tones), the search destination plate and its
note, the controls and their listeners, and the accessibility.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-settings-section = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-settings-section" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/settings_section.rs` lays a section out at Interface
  sizes 12, 16 and 20 and wraps a narrow row.
