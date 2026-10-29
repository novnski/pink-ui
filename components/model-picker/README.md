# Model picker

`pink-model-picker` is PINK's model menu, the card the composer's model chip
opens, as a single-GPUI package (novnski/pink#258). Like the other packages
under `components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/model_picker.rs`), so the gallery's Model Picker and PINK
draw the card from one source. PINK pins the package and patches `gpui` to
its vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `tab_strip`, `tab` and `tab_bar`: the strip of `TAB` squares across the
  top (the Favorites star, the Recent clock, one mark per provider) over a
  hairline, with a 2 pt accent bar under the viewed tab. The bar drops by
  the strip's own inset, so it sits on the hairline at every Interface size.
- `search_row`: the host's magnifier and field over a hairline.
- `list_band`: the model band on a whisper of wash, as tall as the host
  makes it (`LIST_H` at most).
- `model_row` in a `row_slot`: the name, an optional `new_badge` and
  attribution, the host's ⌘1–⌘9 key cap and a `star_button`. A row with a
  `ProviderLine` is the two-line form the Favorites and Recent views use,
  with the provider's mark and name underneath.
- `tray`, `tray_rows` and `setting_parts`: the tray pinned under the list,
  each row's label, value and chevron (the host's menu row is `TRAY_ROW_H`
  tall), and `default_badge` for a choice's default.

Every design size is in pt at the 16 px default and becomes rems through
`ui_rems`, the same formula as PINK's `ui_rems`, so the whole card grows with
the Interface size (PINK sets the window's rem to it).

The host adds what the package can't express with crates.io GPUI: the roles,
accessible names, selection and position, Tab focus and its ring, the hover
washes and tooltips, and the click, hover, keyboard and scroll behaviour. It
also builds the glyphs from its own assets and resolves every colour.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-model-picker = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-model-picker" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- Geometry: `tests/model_picker.rs` in the pink-ui crate lays the strip, a
  compact and a two-line row and the tray out at Interface sizes 12, 16 and
  20 and checks they scale, that the bar stays on the hairline and that a
  long name truncates instead of pushing the star out.
