# Integration Picker

`pink-integration-picker` draws the pieces of PINK's Integration Picker
(Settings, Extensions; issue #374): the integrations on this device, with a
search in the title bar, a Connected and an Available section card of rows,
and the enable review that gives each permission its own switch. Like the
other packages under `components/` it depends on crates.io `gpui = "=0.2.2"`
in its own workspace. The pink-ui gallery compiles this same `src/lib.rs` in
place (`src/components/integration_picker.rs`, the Integration Picker story).
PINK pins the package and patches `gpui` to its vendored runtime, so there is
one GPUI type graph and no copied source.

The settings scaffolding around these pieces (section card, card row, row
text, meta line, badges, switches) comes from
[`pink-settings-parts`](../settings-parts/README.md) and
[`pink-switch`](../switch/README.md).

## What it draws

- `search_field(style, icon, input)`: the title bar's `SEARCH_W` ×
  `SEARCH_H` field with the magnifier glyph and the host's text input.
- `monogram_tile(style, name)`: a row's 28 identity tile carrying the
  integration's initial (`monogram`).
- `identity_line(font, color, text)`: the review's mono line naming exactly
  what is being enabled, such as a package hash.
- `permission_list()` and `permission_row(text, switch)`: the review's
  permissions, one switch each; `permission_detail(detail, required)` words
  a required one's detail.

## The picker's rules

- `arrange(items, query, connected, fields)`: the visible rows as
  `(Section, rows)`, Connected first, each in the order given; a section
  with no match is left out.
- `matches(query, fields)`: a case-insensitive substring match on any field;
  a blank query matches everything.

Every size goes through `ui_rems`, so the pieces grow with the host's
Interface size.

## What the host adds

Ids, roles and accessible names (`Role::SearchInput` on the input, named
buttons and switches), focus, keyboard, hover fills, the actions, the
dialog's modal chrome and every word. The package never invents data: a host
shows what its engine reported, and leaves out a section it has no real
source for.
