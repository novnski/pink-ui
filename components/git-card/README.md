# Git card

`pink-git-card` is PINK's Git card chrome (the Changes pane's Git actions) as
a single-GPUI package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own workspace.
The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/git_card.rs`); Git Actions draws its card through it. PINK
pins the package and patches `gpui` to its vendored runtime, so there is one
GPUI type graph and no copied source.

## What it draws

- `card(id, style)`: the card over the diff, its lines `CARD_GAP` apart
  inside `CARD_PAD_X` / `CARD_PAD_Y`, capped at `CARD_MAX_HEIGHT`.
- `button(id, label, style)`: a `BUTTON_HEIGHT` outlined button; the host
  passes the secondary plate or the accent plate with a medium label.
- `facts`, `note`, `next_step`, `status_row`, `status_state` and
  `status_title`: the branch and remote, notices, the recommended step and
  the pull request's status line.
- `file_list`, `file_row`, `pick_box`, `file_path` and `message`: the changed
  files with their pick boxes and the commit message's slot.
- `section`, `checkpoint_line`, `stack` and `patch`: file checkpoints.
- `panel`, `panel_header`, `pr_title`, `pr_refs`, `pr_description`,
  `check_row` and `field`: the pull request panel and its draft form.
- `button_row()`: a row of buttons.

The host adds the rest: text, inputs, the pull request badge
(`pink-pr-badge`), every resolved colour (each button's plate blended over
its hover fade), the mono family, roles, accessible names, focus rings,
scrolling, clicks, links and every Git and GitHub operation.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-git-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-git-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/git_card.rs` lays the card, a file row and its
  buttons out at Interface sizes 12, 16 and 20.
