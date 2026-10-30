# Prompt library

`pink-prompt-library` draws the pink-ui gallery's Prompt Library as a
single-GPUI package. Like the other packages under `components/` it depends
on crates.io `gpui = "=0.2.2"` in its own workspace. The gallery compiles
this same `src/lib.rs` in place (`src/components/prompt_library.rs`), and
PINK pins the package for the composer's saved prompts: the template rows in
its stash menu and the template being filled in above the input.

## What it draws

- `blank_count(color, count)`: the quiet 11 pt "3 blanks" note at the end
  of a saved template's menu row (`blank_count_label` gives its text).
- `atoms(parts)`: a template's text and blanks as unbreakable groups, so
  words wrap freely and a blank stays glued to the punctuation beside it.
- `sentence(space)` and `group(line_height)`: the wrapping sentence and one
  of its groups.
- `blank_chip(&ChipStyle, width, line_height)`: an editable blank, painted
  as one of the composer's inline chips (`CHIP_RADIUS` corners,
  `CHIP_INSET_Y` inside the line box, `CHIP_PAD_X` side room). The host puts
  its own text field inside.
- `echo_chip(&ChipStyle, text, line_height)`: a repeated blank, showing what
  the first one holds.
- `filled_palette(accent, dark)` and `chip_width(...)`: the filled
  blank's accent chip colours, and a chip as wide as its text. The host
  eases `ChipStyle` between its empty, focused and filled looks.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems, so
  every piece grows with the host's Interface size.

## What the host adds

The template format and its parsing, the text fields, ids, roles and
accessible names, focus, Tab and Enter handling, hover and focus fades,
the menu card and rows it sits in, and every action.
