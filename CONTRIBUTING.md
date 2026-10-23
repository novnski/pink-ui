# Building components

Every component is **one Rust file**, `src/components/<slug>.rs`. A file
depends only on GPUI
and the shared modules in this crate (`motion`, `interaction`, `primitives`,
`pink`, `theme`, `story`), and exports `NAME`, `DESCRIPTION` and `story`.
Icons live in `assets/icons/<slug>/`.

There are two kinds (`components::Kind`):

- **Core** components come in two looks, Clean and Classic.
  `src/components/profile_menu.rs` is the reference: read it first.
- **Pink** components are drawn in Pink's design language. See
  [Pink components](#pink-components).

The looks section applies to core components; the API, state, motion and GPUI
notes apply to both.

## Looks

Each core component comes in two **looks**, and each look in light and dark:

| Look      | Idea                                                         |
| --------- | ------------------------------------------------------------ |
| `Clean`   | One flat card; edges are 0.5px hairline shadows. The default. |
| `Classic` | White cards on a gray tray.                                  |

Build both. Share the structure and logic, and switch tokens and sizes on the
look. The module's `story` is its demo, with realistic sample data.

## API shape (Pink style)

Components are builders you drop into any element tree:

```rust
div().child(
    ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
        .setup(ProfileSetup::new(3, 5).next("Add a profile photo"))
        .on_select(|item, window, cx| { /* ... */ }),
)
```

- `#[derive(IntoElement)] pub struct Name { id: ElementId, …, look: Option<Look> }`
  implementing `RenderOnce`. `Name::new(id, required…)`, chained setters for
  everything optional, and a `.look(Look)` override (default `cx.look()`).
- Callbacks: `on_*(impl Fn(…, &mut Window, &mut App) + 'static)`, stored as `Rc<dyn Fn>`.
- Small, reusable pieces (pills, buttons, rows) can be free functions that
  return `Stateful<Div>` or `Div`, so callers keep chaining (`.child`, `.on_click`).
- Make the public data types (`ProfileMenuItem`, `ProfileSetup`, …) plain
  structs with `new` and setters.

### State

UI state (open/closed, selection, hover index, typed text, timers) lives in
an entity keyed by the component id:

```rust
let state = window.use_keyed_state(
    ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
    cx,
    |window, cx| State::new(window, cx),
);
```

Update it from listeners with `state.update(cx, |s, cx| { …; cx.notify() })`.
The entity lives as long as the component keeps rendering.

- **Text fields**: create a `primitives::TextInput` inside the state entity
  (`cx.new(|cx| TextInput::new(window, cx).placeholder("…"))`), subscribe to
  its `InputEvent`s there and keep the `Subscription` in the state. Render it
  as a child of a styled div: font, size, line height and color are inherited.
  `.multiline()` gives a wrapping text area; `.max_len(1)` suits OTP slots.
  Focus one with `TextInput::focus(&input, window, cx)`.
- **Timers and auto-playing demos**: `cx.spawn(async move |this, cx| { loop {
  cx.background_executor().timer(d).await; this.update(cx, |s, cx| …)?; } })`
  in the state constructor, keeping the `Task` in the state so it stops when
  the component unmounts.
- **Clocks**: take `Instant` and `SystemTime` from `web_time`, never
  `std::time` – std's clocks panic in the browser. Clippy checks this for the
  wasm target (see [Checking your component](#checking-your-component)).

### Colors and sizes

- One `Palette` per look × mode, written like CSS colors (`#171717` →
  `ink: hex(0x171717)`, `rgba(0,0,0,.06)` → `black_a(0.06)`). A `Metrics`
  struct when sizes differ.
- Use exact px values: `px(13.5)`, `.line_height(px(20.))`,
  `.rounded(px(10.))`. Font weights: 400 `NORMAL`, 500 `MEDIUM`,
  600 `SEMIBOLD`, 650 → `FontWeight(650.)`, 700 `BOLD`.
- `font-family: Inter` is already set by the story wrapper and the gallery
  (`primitives::SANS`); use `primitives::MONO` for monospace.
- `box-shadow` → `.shadow(shadows([shadow(x, y, blur, spread, color), …]))`
  with CSS-style values; `0 0 0 1px c` → `ring(1., c)`. For an inset edge,
  use a border.
- `color-mix(in srgb, a 20%, b)` → `color_mix(a, b, 0.2)`.

### Hover, focus and motion

- Hover colors **ease** like the CSS `transition`: use `HoverFill`
  (`.hover_fill`, `.hover_fill_fg`, `.hover_fg`, `.hover_border`,
  `.hover_opacity`) on an element with an `.id(…)`, keyed uniquely
  (`format!("{id}-row-{ix}")`). For child colors that follow the row's hover
  (an icon brightening), read `motion::hover_blend(&key, rest, hover)`.
  Plain `.hover(|s| …)` is fine for things the CSS doesn't transition.
- Keyframe entrances: `motion::rise_in`, `motion::fade_in`, `motion::pop_in`
  or `.with_animation(id, spec.animation(), |el, t| …)`. Ids must be stable.
- Values that glide when they change (thumb positions, heights, fills):
  `motion::follow(key, target, spec)` / `follow_from(key, from, target, spec)`.
- Loops (shimmer, spinners, pulses): `.with_animation(id, Animation::new(d).repeat(), …)`.
- Curves: `motion::EASE_OUT_QUINT` is `cubic-bezier(.22, 1, .36, 1)`, the
  library's standard ease; `EASE`, `EASE_OUT`, `EASE_IN_OUT` match CSS keywords.
- Hosts keep fades and followers moving: the story wrapper adds
  `motion::driver()`, an invisible element that requests frames after
  layout, once every builder has rendered. Add it to any view that hosts
  animated components (or call `motion::keep_alive(window)` at the end of a
  component's own render).

### What GPUI can't do, and the substitutes

| CSS                                   | GPUI                                                   |
| ------------------------------------- | ------------------------------------------------------ |
| `transform: translate`                | `.relative().top(..)/.left(..)` (doesn't move siblings) |
| `transform: scale` on boxes           | animate size, or fade + small translate                |
| `rotate`                              | `svg().with_transformation(Transformation::rotate(..))` |
| `backdrop-filter`, `filter: blur`     | not available – use a translucent fill                 |
| gradients with more than two stops, conic, radial | `canvas()` painting, or layered two-stop `linear_gradient`s |
| masks, `clip-path`                    | `overflow_hidden` + rounded corners, or `canvas()`     |
| `letter-spacing`, `font-variant-numeric` | ignore                                              |
| container / media queries             | design for the desktop size the demo stage shows       |
| `::before` / `::after`                | an extra absolutely positioned `div`                   |
| `margin-left: auto` inside a `gap` row | `flex_1()` + `justify_end()` – in Taffy an auto margin cancels the row's gap |
| charts, rings, dials, waveforms       | `canvas(prepaint, paint)` + `PathBuilder` / `window.paint_quad` |

### GPUI gotchas

Things every component ran into, with the fix that works:

- **Shadows**: use `primitives::shadow`/`ring` with CSS values; they convert
  to GPUI's Gaussian sigma (CSS blur ÷ 2), keep crisp rings drawable (a
  sigma of 0 paints nothing) and turn sub-pixel rings into one pixel. GPUI
  paints a shadow list last-on-top – build CSS-ordered lists with
  `primitives::shadows([...])` – and paints shadows under the whole element,
  so they show through translucent fills.
- **Hairlines**: a `0.5px` border can vanish at 1x; use
  `.border_b(primitives::hairline(window))`.
- **One hover listener per element**: `on_hover` (and so `HoverFill`) can be
  set once. To react to hover yourself as well, combine
  `motion::hover_listener(key)` with your own logic in a single `on_hover`.
- **Enter and Space click** a focused element that has `on_click`; don't also
  handle them in `on_key_down` or the action fires twice.
- **Tab stops**: a `FocusHandle` passed to `track_focus` needs
  `.tab_stop(true)` (on the handle) for `window.focus_next(cx)` to reach it.
- **Truncation**: `truncate()` text measures once; inside a shrinking flex
  item or a `w_full()` child it can collapse to "…". Give it a row parent
  (`flex` → `flex_1().min_w_0().truncate()`) or an explicit px width.
- **Wrapping** counts the trailing space of a line, so text can break one word
  early; a few px of slack on the right brings breaks in line with CSS.
- **Negative margins** in a flex row (overlapping avatars) can measure as zero
  width; give the row an explicit width.
- **`opacity` isn't group opacity**: it applies per layer, and
  `Hsla::opacity` multiplies alpha (it can't make a color more opaque).
- **`motion::rise_in` sets `.relative()`**; for absolutely positioned
  elements, use `with_animation` directly.
- **SVGs don't inherit text color**: always set `.text_color(..)` on `svg()`.

### Icons and images

Put each icon in `assets/icons/<slug>/<name>.svg` with
`xmlns="http://www.w3.org/2000/svg"`, `stroke="currentColor"` /
`fill="currentColor"`, and render it with `svg().path(…).size(..).text_color(..)`
(GPUI tints SVGs as a mask). Strokes with `vector-effect: non-scaling-stroke`
need their width scaled by `viewBox / rendered size` (1.1px at 16px on a
24 viewBox → `stroke-width="1.65"`); keep such variants in a `clean/`
subfolder. Multi-color logos go through `img("icons/<slug>/logo.svg")`.

## Checking your component

```sh
cargo build && cargo test
cargo clippy --lib --target wasm32-unknown-unknown -- -A clippy::all \
  -D clippy::disallowed_types -D clippy::disallowed_methods
scripts/screenshot.sh <slug> light shots/<slug>.gpui.light.png 894x<stage-height> clean
scripts/screenshot.sh <slug> dark  shots/<slug>.gpui.dark.png  894x<stage-height> classic
```

The script renders the gallery stage headlessly (Xvfb + openbox, see the
script). `cargo run -- <slug>` opens the gallery on your component, with
switches for light/dark and Clean/Classic.

A new component shows up on the showcase site by itself: add it to
`components::COMPONENTS` and the catalog, the player and the site pick it up.
Check it in the browser with `scripts/build-site.sh` and
`python3 -m http.server -d site/dist`, then give it a catalog card picture
with `node scripts/site/thumbnails.mjs site/dist` and commit `site/thumbs/`.

## Pink components

- Style with [`crate::pink`](src/pink.rs): `Pink::get(cx)` tokens (surfaces,
  text, borders, fills, accent, status colors), the radius and spacing
  ladders, `pink::SANS` (Geist) / `pink::MONO`, and the control kit
  (`button`, `icon_button`, `chip`, `badge`, `kbd`, `card`, `overlay`).
  Pink components follow light/dark; the Clean/Classic look doesn't apply.
- Use Pink's motion presets (`motion::MENU_IN`, `GLIDE`, `HOVER_FADE`, …)
  and `HoverFill` for hovers.
- Write your own demo content and `DESCRIPTION`; draw icons as simple 24×24
  strokes, `stroke-width="1.5"`.
- Keep the component useful on its own: a clean builder API, real callbacks,
  keyboard support where it makes sense.

## Licensing

Only add code, designs, icons or fonts you have the right to publish under MIT,
and record third-party sources and their licenses in `THIRD_PARTY_NOTICES.md`.
