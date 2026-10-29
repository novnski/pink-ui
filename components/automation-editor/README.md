# Automation editor

`pink-automation-editor` is the layout of PINK's automation editor (the form
that creates or edits a scheduled prompt) as a single-GPUI package (UI
rethink decision 54). Like the other packages under `components/` it depends
on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/automation_editor.rs`,
the Automation Editor story). PINK pins the package and patches `gpui` to its
vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `group(row, &EditorStyle, title)`: a group of the form, the host's settings
  card row as a column of its 12/16 semibold `heading` over its controls,
  `GROUP_GAP` apart.
- `caption(color, text)`: a field's 11/14 caption.
- `frame(&FrameStyle)`: a field's frame, padded `FRAME_PAD_X` by
  `FRAME_PAD_Y` at 13 pt, bordered and filled; the host adds its text input.
- `field(&EditorStyle, label, min, frame)`: the caption over the frame,
  `FIELD_GAP` apart. Fields share a line and wrap below `min`
  (`FIELD_MIN_WIDE`, `FIELD_MIN_DATE`, `FIELD_MIN_SHORT`).
- `setting(&EditorStyle, label, control)`: a label on the `LABEL_WIDTH`
  column with the control beside it; both wrap in a narrow pane.
- `schedule_line(&EditorStyle, icon, words)`: the schedule in words, 13/18
  medium, beside a `SUMMARY_ICON` clock in the accent.
- `disclosure(&EditorStyle, label, chevron)`: the More options toggle, its
  label in the heading style beside the host's chevron, rounded for the
  focus ring.
- `inline()` and `lateness(frame)`: a control and its unit on one line
  (`INLINE_GAP`), and the `LATENESS_WIDTH` lateness field.
- `menu_caption(color, text)`: a select option's 11 pt caption, truncated at
  `MENU_CAPTION_MAX_WIDTH`. `SELECT_MIN_WIDTH`, `SELECT_MAX_WIDTH`,
  `MENU_WIDTH`, `MENU_LIST_MAX_HEIGHT` and `SEGMENTED_WIDTH` size the select
  pop-ups and the New chat / Existing chat track.
- `review(text_min)`: the footer's column of review lines beside its buttons,
  `REVIEW_GAP` apart.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Text,
  padding, gaps and widths grow with the host's Interface size; the frame's
  corners stay fixed.

The rest of the editor comes from packages the host already pins: the
section card and its rows (`pink-settings-section`), buttons and choice chips
(`pink-button`), the segmented track (`pink-segmented-control`), the switch
(`pink-switch`), the select's menu card (`pink-menu-card`) and the model chips
(`pink-trigger-chip`). The host adds the colours, the text inputs, the copy,
the ids, roles and accessible names (a choice chip's toggled state, the
disclosure's expanded state), focus and tab stops, the menus and the actions.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-automation-editor = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-automation-editor" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/automation_editor.rs` lays settings, fields, the
  schedule line and the review column out at Interface sizes 12, 16 and 20.
