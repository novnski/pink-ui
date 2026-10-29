# Queue row

`pink-queue-row` is PINK's message queue as a single-GPUI package (UI
rethink decision 54): the tray tucked behind the composer and the rows of
the messages waiting in it. Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place (`src/components/queue_row.rs`;
the Agent Steering story's tray draws with it). PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

## What it draws

- `tray(&TrayStyle)`: the occluding surface, `TRAY_RADIUS` at the top,
  bordered, with the host's optional shadow and a foot padded only where
  the composer overlaps it (`tucked`).
- `row(&RowStyle)`: one message, `ROW_HEIGHT` tall and fixed at every
  Interface size so drop slots line up, padded `ROW_PAD_X`, `ROW_RADIUS`
  corners, its parts `ROW_GAP` apart (`COMPACT_ROW_GAP` in a narrow
  composer), the host's wash, faded while it is removed.
- `drag_marker(glyph, color, blocked)` and `edit_spacer()`: the
  `MARKER_WIDTH` by `MARKER_HEIGHT` marker, dimmed when the row can't move,
  and the space that keeps the text on its column while the row is edited.
- `content()`, `message_line(text)`, `status(color, label)`,
  `message(text)`, `summary(color, text)`, `editing_note(color, text)`: the
  12.5/16 message line with its status in front, the 11/13 attachment
  summary under it, and what an edited row says instead.
- `actions()`, `action(&ActionStyle)`, `action_glyph(glyph, color)`,
  `primary(&ActionStyle, compact)`: the trailing group, the `ACTION_SIZE`
  discs (Remove, Edit, Save, Cancel) and the `PRIMARY_WIDTH` Send now or
  Steer now capsule (a disc in a compact row), dimmed while blocked.
- `thumbnail(&ThumbnailStyle)`, `thumbnail_image(source)`,
  `thumbnail_placeholder(&ThumbnailStyle, glyph)`, `more(bg, text, label)`:
  the `THUMB_WIDTH` by `THUMB_HEIGHT` image frame, its cover-fit image, the
  glyph while a preview loads or failed, and the `+N` count.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Text
  grows with the host's Interface size; the row, marker, discs and
  thumbnails stay fixed.

The host adds the rest: the resolved colours (washes and labels blended over
their hover fades), the glyphs and copy, the ids, hover listeners, roles,
accessible names, focus rings and tooltips, the rows' scroller and edge fade,
drag and drop with the slide animation, and the queue actions.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-queue-row = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-queue-row" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/queue_row.rs` lays a wide and a compact row out at
  Interface sizes 12, 16 and 20.
