# Row card

`pink-row-card` is PINK's transcript row card as a single-GPUI package (UI
rethink decision 54): the bordered card a conversation shows for a helper
agent or an automation, with the settings row anatomy. Like the other
packages under `components/` it depends on crates.io `gpui = "=0.2.2"` in
its own workspace. The pink-ui gallery compiles this same `src/lib.rs` in
place (`src/components/row_card.rs`; the Subagent Cards story draws with
it). PINK pins the package and patches `gpui` to its vendored runtime, so
there is one GPUI type graph and no copied source.

## What it draws

- `card(Layout, &CardStyle)`: the bordered, padded card (`PAD_X` by
  `PAD_Y`) holding the tile, the text column and the actions. `Layout::Wrapping`
  (`WRAPPING_RADIUS`) wraps the actions under the text when the text would
  fall below `WRAPPING_TEXT_MIN`; `Layout::Inline` (`INLINE_RADIUS`) keeps one
  line.
- `text_column(Layout)`: the title line over the detail line, `LINE_GAP`
  apart.
- `title_line()`, `title(color, text)`, `type_badge(&BadgeStyle, label)`: the
  13/18 medium title and the quiet `BADGE_HEIGHT` type badge after it.
- `detail(color, text)`: a one-line 12/16 detail.
- `meta_line(color)`, `status(dot, tone, label)`, `fact(text)`,
  `preview(color, text)`: a detail line of separate facts `META_GAP` apart (a
  status dot and its word, facts that never shrink, and a preview that alone
  truncates). Space sets the facts apart, never a separator glyph.
- `actions(Layout)`: the trailing column of buttons.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Padding,
  gaps and text grow with the host's Interface size; the corners stay fixed.

The tile is `pink-settings-parts` `row_tile`. The host adds the rest: the
resolved colours, the tile's glyph, the copy and buttons, the ids, roles and
accessible names, tooltips and the actions.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-row-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-row-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/row_card.rs` lays wide, narrow and inline cards out
  at Interface sizes 12, 16 and 20.
