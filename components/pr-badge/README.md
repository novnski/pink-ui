# PR badge

`pink-pr-badge` is PINK's pull request badge as a single-GPUI package (UI
rethink decision 54). Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/pr_badge.rs`); the Git Actions card's PR badge and the Chat
Composer's PR chips draw through it. PINK pins the package and patches
`gpui` to its vendored runtime, so there is one GPUI type graph and no
copied source.

## What it draws

- `badge(Surface, &BadgeStyle)`: the frame. `Surface::Sidebar` is the
  compact badge a sidebar row or task list carries (`SIDEBAR_HEIGHT`,
  `SIDEBAR_TEXT`, `SIDEBAR_PAD_X`, `SIDEBAR_RADIUS`); `Surface::Composer` is
  the taller chip in the composer's context row (`COMPOSER_HEIGHT`,
  `COMPOSER_TEXT`, `COMPOSER_PAD_X`, `COMPOSER_GAP`, `COMPOSER_RADIUS`).
  Both use a medium label; `BadgeStyle::fill` is the state-toned plate
  (`None` for the task list's bare number) and `text` the label colour.
- `glyph(path, color)`: the `GLYPH` pull request glyph a composer chip
  leads with.
- `number(mono, number)`: the number in the host's mono family, so its width
  stays stable as numbers change; `number` may be a highlighted search
  match. A plain word (Draft) goes in as a text child.
- `ui_rems(design)`: heights, label and glyph sizes are rems (pt at the
  16 px default), so a badge follows the host's Interface size; padding,
  gaps and corners stay in pixels.

The host adds the rest: the state's tone resolved for legibility, the plate
and label already blended over its hover fade, the glyph path, the mono
family, and the id, role, accessible name, cursor, tooltip and click.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-pr-badge = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-pr-badge" }
```

PINK's `change_requests.rs` (`pull_request_badge_with_query`,
`task_pull_request_badge`, `composer_task_pull_request_chip`) are adapters
over it, and `composer_task_pull_request_chrome` reserves the chip's room
from its constants.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/pr_badge.rs` lays the badges out at Interface sizes
  12, 16 and 20, checks the composer chip against the room
  `pull_request_chrome` reserves, and squeezes a chip in a crowded row.
