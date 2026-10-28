# Command row

`pink-command-row` is PINK's command palette rows as a single-GPUI package
(UI rethink decision 54). Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/command_row.rs`). PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

A row's base is the host's menu row (the `pink-menu-card` row with its
selected wash or hover fade); this package shapes it for the palette.

- `row(base, RowKind)`: the palette's concentric `ITEM_RADIUS` and the
  kind's height and padding: `Action` (32 pt, one line), `Detail` (44 pt, a
  matching thread's two lines) or `More` (32 pt, centred).
- `slot(first, last)` and `inset(row)`: a result's place in the list, with
  the room above the first and below the last result, and its inset.
- `glyph(svg, color)`: the 16 pt leading glyph.
- `label()`: the room the highlighted label takes.
- `note(text, color)`: an extension command's name after its label.
- `detail(summary, meta, &DetailStyle)`: a thread's summary over its
  attribution.
- `heading(color)`: a section's quiet heading.
- `separator(element)`: spaces the host's separator between sections.
- `status(color)`: the status line under the results.
- `key_hint(keys, label, color)`: a footer key hint beside the host's kbd.

Every size is in `ui_rems(design)`, so the palette grows with the host's
Interface size.

The host adds the rest: the colours, the glyphs, the highlighted label,
the kbd hints, the ids, roles and accessible names, the click and keyboard
handling, the scroller and its fades.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-command-row = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-command-row" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/command_row.rs` lays rows out at Interface sizes
  12, 16 and 20.
