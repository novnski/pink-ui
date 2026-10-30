# Agent plan

`pink-agent-plan` is the Agent Plan (PINK issue #370, part of #266) as a
single-GPUI package (UI rethink decision 54): the steps an agent proposes
before it runs, shaped by the person in a tray on the composer, then followed
as they run. Like the other packages under `components/` it depends on
crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles
this same `src/lib.rs` in place (`src/components/agent_plan.rs`, the Agent Plan
story). PINK pins the package and patches `gpui` to its vendored runtime, so
there is one GPUI type graph and no copied source.

The tray, its rows and their action discs are
[`pink-queue-row`](../queue-row/README.md); a failed step's notice is
[`pink-notice`](../notice/README.md)'s danger chip. This package draws the
plan's own pieces.

## What it draws

- `header()`, `header_summary(style, glyph, label, objective, facts)` and
  `controls()`: the tray's `HEADER_H` bar (drawn like the goal card's): the
  glyph, the medium "Plan", the objective (which alone truncates) and the
  trailing facts, apart on the row's gap rather than joined by a separator;
  the controls hold the host's Run plan.
- `step_text(style, title, detail)` and `step_title(color, title)`: a step's
  title over its detail line (the tool and estimate, or what the provider did
  not share) inside a `ROW_H` queue row. `field(color)` holds a rename or add
  field in the same column.
- `slot(glyph, color)` and `add_label(color, label)`: the quiet "Add a step"
  row's glyph and label.
- `checklist(style)`, `checklist_row()`, `disclosure(style, chevron, summary)`,
  `close(style, glyph)`, `lines()`, `line(style, status, title)` and
  `note(text)`: the running plan (drawn like the provider checklist): a
  `CHECKLIST_ROW_H` disclosure reading "2 of 5, In progress: step" beside a
  `CLOSE` tile, unfolding into one "Status: step" line per step, the failed
  word in red, and a closing note.
- `StepStatus` with its status `word()`, and `summary(steps)`: the run's
  summary from each step's status and title, with the range of the red word.
- `drop_index`, `slide_offset`, `drag_offsets` and `move_step`: the drag
  geometry (in slots) and the reorder a drop makes.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems.

## What the host adds

Resolved colours (hover fills already blended), the glyph paths, the copy and
the step data; the ids, roles (status, buttons, list and list items) and
accessible names; focus rings, tooltips, the text fields, drag and drop, the
slide animation and scrolling; and every action: rename, reorder, add,
remove, Run plan, Stop, retry and skip.

## Checks

`tests/agent_plan.rs` (pink-ui root) lays the header, a step row and the
running checklist out at Interface sizes 12, 16 and 20 and checks their
heights, that a long objective or step title never pushes Run plan or a row's
actions out, that a long step line wraps, the run summary's wording and red
word, and that a reorder never drops a step.
