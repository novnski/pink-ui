# Section header

`pink-section-header` is PINK's sidebar section and shelf headers as a
single-GPUI package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/section_header.rs`), and the Sidebar draws its group and
shelf headers with it. PINK pins the package and patches `gpui` to its
vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `disclosure_header(scale)`: the quiet header over a group of rows,
  `header_height(scale)` tall (28 pt grown with the Interface size, never
  under the 24 pt `MIN_HIT`), `HEADER_PAD_X` padding, `HEADER_RADIUS`
  corners, no rule. `disclosure_label(color, text)` is its 11 pt semibold
  label and `reveal_on_hover(group)` holds the chevron that appears only
  under the pointer.
- `shelf_header(scale)`: a bottom shelf's header at the same height, its
  parts `SHELF_GAP` apart: `mark_slot()` (the 16 pt mark slot every sidebar
  row shares, so the label starts on the title column), `shelf_label(text)`
  (12 pt medium), `shelf_count(color)` (11 pt) and `trailing_chevron()`,
  whose glyph's empty space is pulled back `CHEVRON_TRAILING_BEARING` so the
  arrow ends on the time column.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems.

The host adds the rest: the glyphs and the chevron's rotation, the colours
(held to their contrast floors), the label's edge fade, the count's tabular
figures, the id, the hover fill and its fade, the role, accessible name,
expanded state, focus and the toggle.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-section-header = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-section-header" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/section_header.rs` lays both headers out at
  Interface sizes 12, 16 and 20.
