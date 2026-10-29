# Empty State

`pink-empty-state` is PINK's empty and error states as a single-GPUI package
(UI rethink decision 54). Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/empty_state.rs`); its Sidebar, Agent Inbox and Decision
Inbox draw their empty lists with it. PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `note(&NoteStyle, title, hint)`: the list note PINK's sidebar shows with
  nothing in it, a centred medium `NOTE_TITLE` title over a `NOTE_HINT` hint,
  `NOTE_GAP` px apart inside fixed `NOTE_PAD_X` by `NOTE_PAD_Y` px insets.
- The pane state PINK's Browser surface shows before a page or after one
  failed to load:
  - `pane()`: fills the surface and centres the column, padded `PANE_PAD`.
  - `column()`: the centred column, `PANE_GAP` apart, never wider than
    `PANE_MAX_WIDTH`.
  - `tile(&TileStyle)`: the `TILE` glyph tile, `TILE_RADIUS` px corners, for
    a `TILE_GLYPH` glyph.
  - `title(color)` and `description(color)`: the medium `TITLE` title and the
    centred `DESCRIPTION` / `DESCRIPTION_LINE` description.
  - `action(&ActionStyle)` and `shortcut(color)`: the bordered
    `ACTION_HEIGHT` action and the shortcut after its label.
- The place state PINK's right rail shows in a place with nothing open:
  - `place()`: fills the place and centres its parts `PLACE_GAP` apart,
    padded `PLACE_PAD` px.
  - `place_glyph(glyph, color)`: sizes the host's place glyph to a fixed
    `PLACE_GLYPH` px and tints it.
  - `place_message(color)` and `place_notice(color)`: the `PLACE_MESSAGE`
    line and the centred `PLACE_NOTICE` notice, no wider than
    `PLACE_NOTICE_WIDTH` px. The host adds its own button after them.
- The guide PINK's Related place shows before any related chat exists:
  - `guide(&GuideStyle)`: the `GUIDE_TEXT` column, `GUIDE_GAP` apart,
    `GUIDE_PAD_X` px by `GUIDE_PAD_Y` padded.
  - `guide_title(&GuideStyle)`: the heading in the title ink.
  - `guide_line(&GuideStyle, glyph, text)`: the host's kind glyph, sized
    `GUIDE_GLYPH` and dropped `GUIDE_GLYPH_DROP` px onto the first line,
    `GUIDE_GLYPH_GAP` before its wrapping explanation.
  - `guide_action()`: the slot for the host's button, `GUIDE_ACTION_SPACE`
    px lower.
- `popup_note(color, copy)`: the centred muted `POPUP_NOTE_TEXT` line of an
  empty popover list ("No models found"), padded `POPUP_NOTE_PAD_X` by
  `POPUP_NOTE_PAD_Y`.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Every
  pane size grows with the host's Interface size; the radii, the list
  note's insets, the place state's padding and glyph and the guide's side
  padding stay fixed.

The host adds the rest: the resolved colours, the glyph, the copy, the ids,
the status role and accessible names, the action's hover fill and the
listeners.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-empty-state = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-empty-state" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/empty_state.rs` lays every shape out at Interface
  sizes 12, 16 and 20.
