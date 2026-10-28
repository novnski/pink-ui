# Question card

`pink-question-card` is PINK's question card as a single-GPUI package (UI
rethink decision 54): the panel an agent's question, approval or permission
request takes in the composer's place (PINK composer.rs `render_wizard`).
Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/question_card.rs`), and its
Approval Card and Permission Request stories draw from it. PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI
type graph and no copied source.

## What it draws

- `body()`: the stack inside `PAD`, holding the parts below in order.
- `header(&Palette, title, counter)`: the host's tracked uppercase title
  and, on a paged request, the counter chip.
- `question(id, &Palette, text)`: the question, scrolling past
  `QUESTION_MAX_HEIGHT`.
- `context(id, &Palette, rows)`: the approval context as quiet labels over
  their values, scrolling past `CONTEXT_MAX_HEIGHT`.
- `hint(&Palette, text)`: the multi-select hint.
- `options(id)` and `option(&OptionStyle, label, details, number)`: the
  scrolling option list and one row, its label over its detail lines and the
  number chip naming its 1 to 9 key, rounded to `OPTION_RADIUS`.
- `answer(surface, field)`: the host's nested surface inset by
  `ANSWER_INSET`, holding the field padded by `ANSWER_PAD_X` by
  `ANSWER_PAD_Y`.
- `footer()`, `notes()` and `note(&Palette, text)`: the action row, its
  leading slot for notes and secondary actions, and one note. The primary
  action keeps the trailing edge, padded by `ACTION_PAD_X`.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems.
  Padding, gaps and text grow with the host's Interface size; corners and
  the context rows' 2 px gap stay fixed.

The host adds the rest: the card's surface (PINK's composer pill), the
colours already resolved (each option's fill blended over its hover fade,
the picked wash), the cased header text, the buttons, the ids of unscrolled
parts, roles and accessible state, focus rings, hover listeners, keyboard
shortcuts and the actions.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-question-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-question-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/question_card.rs` lays the card out at Interface
  sizes 12, 16 and 20.
