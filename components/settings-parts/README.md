# Settings parts

`pink-settings-parts` is PINK's settings row parts as a single-GPUI package
(UI rethink decision 54), beside `pink-settings-section`'s card and rows.
Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/settings_parts.rs`); the kit's
`pink::row_tile`, the Appearance Picker's theme cards, the Settings Page's
warning strip, the Integration Settings meta line and the Dropzone's error
strip draw through it. PINK pins the package and patches `gpui` to its
vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `row_tile(&TileStyle, icon_path)`: a row's identity tile, `TILE_SIZE`
  square with `TILE_RADIUS` corners, a 1 px border, a quiet fill and a
  centred icon. Callers may add a status dot as a child.
- `meta_line(color, fragments)`: the quiet facts line under a row title,
  fragments separated by space (never by a glyph), wrapping inside the row.
- `option_card_row()` and `option_card(&OptionCardStyle, icon_path, label,
  selected, preview)`: equal preview cards for picking a visual option, an
  `OPTION_CARD_HEIGHT` frame inside a `selection_ring` and a caption. The
  caption glyph takes the label's colour (`text` when chosen, `text_muted`
  otherwise): an svg paints only with its own text colour.
- `error_strip(&StripStyle, icon_path, message)` and
  `warning_strip(&StripStyle, icon_path, message)`: the red and amber strips
  a card shows above or between its rows, a leading icon before wrapping
  copy.
- `ui_rems(design)`: every size next to text is a rem (pt at the 16 px
  default), so a page follows the host's Interface size; hairlines, corners
  and the selection ring stay in pixels.

The host adds the rest: the resolved colours (the readable tones, the strip
plates and edges, the selection accent), the icon paths and option
previews, and the ids, roles, accessible names, focus rings and listeners.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-settings-parts = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-settings-parts" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/settings_parts.rs` lays the parts out at Interface
  sizes 12, 16 and 20 and wraps a meta line in a narrow row.
