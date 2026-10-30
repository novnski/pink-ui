# Agent tree

`pink-agent-tree` is the outline around PINK's nested Agents list as a
single-GPUI package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own workspace.
The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/agent_tree.rs`, the Agent Tree story). PINK pins the package
and patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

The rows are [`pink-agent-row`](../agent-row/README.md); this package nests
them.

## What it draws

- `indent(depth)`: a row's leading padding at a depth. A child's status dot
  sits `INDENT` right of its parent's and starts `RAIL_SPAN` right of the
  parent's trunk, where its branch ends `BRANCH_GAP` short of it.
- `Rail`, `Rail::settled(depth, ancestors_continue, open)` and
  `rail(rail, color, scale)`: the activity rail over one slot (a row and the
  `ROW_GAP` under it, `SLOT_H` tall): ancestor trunks that still have a later
  sibling, the elbow (`BEND_RADIUS`) off the parent's trunk with its
  continuation for a later sibling, and the row's own trunk while its branch is
  open. Fully drawn segments share one nonzero path so joints never double
  their alpha; partly drawn ones (`incoming`, `branch`, `start`'s drawn share)
  let a host animate arrivals. Paint it in the hairline colour at
  `RAIL_ALPHA`. `scale` is the window's rem size over 16.
- `paint_rail` and `branch_points`: the same, for a host's own canvas.
- `disclosure(icon, open, color, group_hover)`: the `TILE` disclosure tile with
  its `CHEVRON`, turned 0 to 90 degrees by `open`, optionally recoloured
  while the pointer is over a row group. `tile_slot()` is a leaf's empty slot,
  so status words end on one column.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems.

## What the host adds

Resolved colours, the chevron's icon path, the fold progress and how much of
each rail is drawn; the ids, tree roles (`TreeItem`, level, expanded,
selected) and accessible names; focus, the arrow-key walk, hover and
selection fills; and every action (open, fold, Stop, Retry).

## Checks

`tests/agent_tree.rs` (pink-ui root) lays three nested rows out at Interface
sizes 12, 16 and 20 and checks each dot against its parent's branch, the
tile's slot and the settled rails.
