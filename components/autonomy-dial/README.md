# Autonomy Dial

`pink-autonomy-dial` draws the body of PINK's Autonomy Dial (issue #372): an
agent's actions sorted into the groups its permission level puts them in,
"Unattended" and "Needs approval". Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own workspace.
The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/autonomy_dial.rs`, the Autonomy Dial story). PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI type
graph and no copied source.

The level is PINK's own access chip and Permission mode menu
(`pickers.rs`); the dial's groups sit under the menu's levels.

## What it draws

- `groups()` and `group()`: the column the groups stack in, `GROUP_GAP`
  apart, and one group's column.
- `heading(style, title, count)`: a `HEAD_H` group heading, the name cut with
  an ellipsis and the count at the end. `HeadingStyle` carries the resolved
  colours.
- `row(style, icon, label, meta)`: a `ROW_H` action row on the sidebar's
  leading grid: the glyph in its `MARK_SLOT` (kept empty without one, so
  names line up), the name cut with an ellipsis, and the meta word (who
  decides) at the end, which never gives way. `RowStyle` carries the
  resolved colours. A host may append a trailing control, such as a lock.
- `empty(color, text)`: the quiet line an empty group holds.

Every size goes through `ui_rems`, so the pieces grow with the host's
Interface size.

## What the host adds

Ids, roles and accessible names (`Role::Heading` on a heading, named with
its count in words), focus rings, hover fills, tooltips, any motion (the
gallery glides rows between groups), and every word. The package never
invents a classification: a host shows what its engine says each action
does, and says so when a provider's policy can't be read per action.
