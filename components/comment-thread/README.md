# Comment thread

`pink-comment-thread` is PINK's review comment thread as a single-GPUI
package (UI rethink decision 54, PINK issue #373): an inline review comment
grown into a conversation, with replies, their authors, the agent's reply
leading back to its turn, and resolved state. Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/comment_thread.rs`). PINK pins the package and patches
`gpui` to its vendored runtime, so there is one GPUI type graph and no
copied source.

The thread sits on `pink-comment-card`'s band and header; this package draws
what grows inside it.

## What it draws

- `meta(avatar, name, time, &MetaStyle)`: the `AVATAR` row with the author's
  disc, the medium name (truncating) and the time, which never wraps.
- `initial_avatar(initial, &AvatarStyle)` and `mark_avatar(svg, &AvatarStyle)`:
  the disc with a person's initial (`initial(name)`) or the agent's mark.
- `body(text, color, lines)`: the note, indented by `TEXT_INDENT` under the
  name and exactly `lines` tall, clipping past it.
- `plate(svg, label, &PlateStyle)`: the agent reply's summary plate (the
  transcript's tool-group plate), indented under the name; the host makes it
  the button that opens the turn.
- `message()` and `messages()`: one message's column and the conversation,
  `MESSAGE_GAP` apart.
- `count(label, color)` with `count_label(n)`, and `chip(svg, label,
  &ChipStyle)`: the header's message count and the Resolved chip.
- `footer()`: the trailing action row for Reply, Resolve or Reopen (draw the
  actions with `pink-comment-card` `action`).
- `thread_height(&[MessageLines], footer, ui_font_size)` and
  `message_height`: the band's worked-out height, so a diff can tween a fold
  over it. Fixed gaps stay; the header, meta rows, notes and plates scale.
- `ui_rems(design)`: a design size in rems.

## What the host owns

Every resolved colour (hover fills already blended), the glyphs, the names,
times and text, the band, ids, roles, accessible names, focus, keys,
tooltips and every listener. Thread state, replies and resolution stay with
the host; the package never invents a message, a name or a time.

## Checks

```sh
cargo clippy --features gpui/runtime_shaders -- -D warnings
```

The geometry test is `tests/comment_thread.rs` in the gallery
(Interface sizes 12, 16 and 20).
