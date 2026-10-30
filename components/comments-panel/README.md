# Comments panel

`pink-comments-panel` is PINK's Comments Panel as a single-GPUI package (UI
rethink decision 54, PINK issue #387): a document's review threads listed in
a side pane, with the open count and an Open / Resolved switch. Like the
other packages under `components/` it depends on crates.io `gpui = "=0.2.2"`
in its own workspace. The pink-ui gallery compiles this same `src/lib.rs` in
place (`src/components/comments_panel.rs`). PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

Each entry sits on `pink-comment-card`'s band and header; this package draws
the pane around them and what an entry shows under its header. The
gallery's slash-command edit cards and draft card stay in the gallery: a
host shows only the threads it has.

## What it draws

- `pane(&PaneStyle)`: the full-height column the host fills.
- `toolbar()`, `heading(title, color)` and `count(label, color)`: the
  `TOOLBAR_HEIGHT` row, the title on the entries' text column
  (`TITLE_INSET`) and the open count; the host adds its `SWITCH_WIDTH`
  Open / Resolved switch after the heading.
- `list()` and `entry()`: the entries' column and one entry's slot,
  `ENTRY_GAP` apart.
- `content()`: the column inside an entry's band.
- `byline()`, `name(text, color)` and `time(text, color)`: who opened the
  thread and when; the name truncates, the time never wraps.
- `note(text, color, lines)`: the opening note, exactly `lines` tall.
- `replies(label, color)` with `replies_label(n)`: the replies line.
- `entry_height(lines, replies, ui_font_size)`: the band's worked-out
  height. Fixed padding stays; the header, byline, note and replies scale.
- `ui_rems(design)`: a design size in rems.

## What the host owns

Every resolved colour (hover fills already blended), the glyphs, the names,
times and text, the band, the switch, ids, roles, accessible names, focus,
keys, tooltips, scrolling and every listener. Which tab is shown and the
threads themselves stay with the host; the package never invents a thread,
a name or a time.

## Checks

```sh
cargo clippy --features gpui/runtime_shaders -- -D warnings
```

The geometry test is `tests/comments_panel.rs` in the gallery
(Interface sizes 12, 16 and 20).
