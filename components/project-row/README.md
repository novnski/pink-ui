# Project row

`pink-project-row` is PINK's project picker rows as a single-GPUI package
(UI rethink decision 54). Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/project_row.rs`). PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

A row's base is the host's keyboard-navigable menu row (the `pink-menu-card`
row with its highlight and selection check) and its leading tile is the
host's project monogram (`TILE`, `ALL_GLYPH`); this package lays out what
follows.

- `text_column()`: the name line over the folder line.
- `name_line(name)`: the medium truncating name; the host adds the tag and
  the disconnected glyph.
- `tag(text, color)`: the device tag, at most `TAG_MAX_SHARE` of the line.
- `glyph(svg, color)`: a 12 pt glyph (the disconnected mark, a footer
  action's glyph).
- `subtitle(text, color, font_family)`: the folder (mono) or All projects'
  sentence.
- `counts()`, `count(glyph, n, color)`, `working_mark(color)`,
  `last_active(text, color)`: the trailing activity (approvals waiting,
  tasks running) or when the project was last active. The host adds the
  tabular figures.
- `action_label(text)`: a footer action's label.
- `divider(color)`: the pinned footer's divider, full-bleed through the
  card's inset.
- `empty(text, color)`: the line shown when no project matches.
- `LIST_GAP` and `LIST_MAX_HEIGHT`: the list's rhythm and scroll budget.

Sizes are in `ui_rems(design)`, so the rows grow with the host's Interface
size; the divider and the empty line's padding stay in px.

The host adds the rest: the colours already held to their contrast floors,
the glyphs, the mono family, the ids, listbox roles and accessible names,
the pointer and keyboard cursor, the clicks and the context menu.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-project-row = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-project-row" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/project_row.rs` lays rows out at Interface sizes
  12, 16 and 20.
