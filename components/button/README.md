# Button

`pink-button` is PINK's control kit frame (`button.rs`) as a single-GPUI
package (UI rethink decision 54): the one frame every push button, glyph
square, pop-up button and chip shares, in three sizes. Like the other
packages under `components/` it depends on crates.io `gpui = "=0.2.2"` in its
own workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/button.rs`), and the gallery kit's `pink::button`,
`icon_button`, `select` and `chip` draw through it. PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and no
copied source.

## What it draws

| Size | Height | Radius | Type  | Side pad | Glyph | Gap |
| ---- | ------ | ------ | ----- | -------- | ----- | --- |
| `Sm` | 24     | 6      | 11/14 | 8        | 12    | 4   |
| `Md` | 28     | 8      | 12/16 | 10       | 14    | 6   |
| `Lg` | 32     | 8      | 13/18 | 12       | 16    | 6   |

- `ControlSize`: the ladder above, in pt at the 16 px default.
- `frame(id, &ControlStyle)`: one row of centred content at the size's
  height, gap, corner, type and line height inside a 1 px edge, with the
  host's plate, label colour, edge and weight. An enabled control shows the
  pointing cursor; a disabled one keeps its size, takes the default cursor
  and rests at `DISABLED_OPACITY`.
- `button` (padded by the side pad), `square` (as wide as it is high) and
  `popup` (leading layout, the chevron `POPUP_GAP` after the choice and its
  side `POPUP_TRAIL_TIGHTEN` tighter): the frame's three shapes.
- `ui_rems(design)`: a design size in rems. Heights, padding, gaps and type
  grow with the host's Interface size; corners and the edge stay in pixels.

The host adds the rest: the kind's colours mixed over its hover fade, the
children (label, glyphs tinted by the host), the role, accessible name and
toggled/expanded state, Tab focus and its ring, the hover listener, the
Enter/Space handling and the actions.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-button = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-button" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/button.rs` lays every size out as a button, a
  disabled button, a square and a pop-up at Interface sizes 12, 16 and 20.
