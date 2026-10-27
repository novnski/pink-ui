# List row and status indicator

`pink-list-row` is PINK's sidebar row and status indicator as one
independently consumable package (UI rethink milestone 1, decisions 54 to 56,
novnski/pink#173). It takes the same route as `components/tool-activity`: the
gallery stays on gpui-pre, PINK stays on its patched GPUI, and PINK pins this
package and patches crates.io `gpui` to its existing runtime. There is one GPUI
type graph and no source copying: the pink-ui gallery compiles this same
`src/lib.rs` in place (`src/components/list_row.rs`) for its List Row story.

## What it draws

- `list_row(id, ListRow, &RowStyle)`: a row 32 pt tall for one line and 52 pt
  for two (`row_height`), scaled with the Interface size and never under the
  24 pt minimum hit area. The title starts `ROW_INSET` (8) in from the row's
  edge and the trailing corner ends 8 in from the other edge, so a selected
  row's text sits balanced in its wash and every scope shares one title
  column (no indent under a group header). An optional leading mark sits in
  the 16 pt mark slot, 8 before the title. The trailing corner has a fixed
  width (`RowStyle::corner_width`): the time at rest, the host's actions
  under the pointer, so hover never moves or squeezes the title. One selected
  style.
- `status_indicator(Signal, glyph, compact, &RowStyle)`: Running (the host's
  spinner, or a 6 pt busy square), Unread (a 6 pt dot), and a pill naming the
  next step for Waiting for input (Reply), Needs approval (Approve) and Failed.
  Each `Signal` has a spoken name; `Signal::Running.needs_you()` is false, so
  running is never shown or spoken as needing the user.
- `ListRow::accessible_name()`: the title and its signal ("Retire expired
  coupon codes, Waiting for your approval").

The host supplies theme tokens (`RowStyle`, already floored for contrast), the
Interface scale, glyphs, the secondary line, chips, hover and selection state,
and actions. The package has no stored state, timers or app logic. The title
and corner carry the debug selectors `{id}-title` and `{id}-corner` for
geometry checks.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside `pink-tool-activity`:

```toml
pink-list-row = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-list-row" }
```

PINK's root `Cargo.toml` already patches crates.io `gpui` to
`vendor/zui/crates/gpui`, which this package resolves to as well. A thin
adapter in the sidebar maps PINK's `Theme` to `RowStyle`
(`theme.ui_scale()`, `text`, `readable_muted`, `sidebar_hover_bg`,
`glass_selected_bg`, `busy`, the floored warning and danger tones) and
`ChatIndicator` to `Signal`, then adds what the runtime owns:

```rust
let name = row.accessible_name();
let selected = row.selected;
pink_list_row::list_row(id, row, &style)
    .role(gpui::Role::Button)
    .aria_label(name)
    .aria_selected(selected)
    .tab_index(0)
    .on_hover(/* corner hover state */)
    .on_click(/* open the thread */)
```

crates.io GPUI 0.2.2 has no accessibility API, so the semantics stay with the
host, as for `pink-tool-activity`. Adoption, including deleting the app's
local row implementation (decision 55), is tracked on novnski/pink#173.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory (crates.io `gpui 0.2.2`; `runtime_shaders` only avoids the Metal
  toolchain on macOS).
- Against PINK's runtime: a host crate that depends on this package by path
  and patches `gpui` to PINK's `vendor/zui/crates/gpui` with PINK's lockfile
  (`cargo check`, `cargo clippy -p pink-list-row -- -D warnings`).
- Geometry and signals: `tests/list_row.rs` in the pink-ui crate lays the rows
  out at Interface sizes 12, 16 and 20 and checks one title column at rest,
  hovered, selected and on two lines, a fixed corner, balanced insets,
  heights, and separate, named signals.
- Formatting: `rustfmt --edition 2024 --check src/lib.rs`.
