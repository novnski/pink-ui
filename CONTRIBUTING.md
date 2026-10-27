# Building components

Every component is **one Rust file**, `src/components/<slug>.rs`, drawn in
the Pink app's design language. A file depends only on GPUI and the shared
modules in this crate (`pink`, `motion`, `interaction`, `primitives`, `theme`,
`story`), and exports `NAME`, `DESCRIPTION` and `story`. Icons live in
`assets/icons/<slug>/`.

Read [`docs/pink-design.md`](docs/pink-design.md) first: it is the design
language (tokens, ladders, recipes, motion, what Pink avoids) with the kit call
for each piece. When Pink already has the thing you are building, copy it: read
its render code in the [Pink app](https://github.com/novnski/pink)
(`crates/ui/src`) and reproduce it value for value, naming the Pink source in
the doc comments. `src/components/thinking_loader.rs` (Pink's working line) and
`src/components/tool_call_card.rs` (its tool group) are good references.

## API shape

Components are builders you drop into any element tree:

```rust
div().child(
    ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
        .setup(ProfileSetup::new(3, 5).next("Add a profile photo"))
        .on_select(|item, window, cx| { /* ... */ }),
)
```

- `#[derive(IntoElement)] pub struct Name { id: ElementId, … }` implementing
  `RenderOnce`. `Name::new(id, required…)` and chained setters for everything
  optional.
- Callbacks: `on_*(impl Fn(…, &mut Window, &mut App) + 'static)`, stored as `Rc<dyn Fn>`.
- Small, reusable pieces (pills, buttons, rows) can be free functions that
  return `Stateful<Div>` or `Div`, so callers keep chaining (`.child`, `.on_click`).
- Make the public data types (`ProfileMenuItem`, `ProfileSetup`, …) plain
  structs with `new` and setters.
- The module's `story` is its demo, with realistic sample data.

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

- Start a render with `let t = Pink::get(cx);`: the app's tokens for the active
  light/dark mode (surfaces, text tiers, borders, fills, accent, status). Never
  hard-code colors; secondary text goes through `t.readable_muted(..)` and
  friends, fills through `t.fill(FillTier::…)`, edges through `t.border`,
  `t.separator(..)` or `t.hairline(..)`.
- Sizes come from Pink's ladders: `pink::radius`, `pink::space`, `pink::text`
  and `TextStyle`, `pink::icon_size`, `ControlSize`, and the `layout`
  constants. Use exact px only when Pink's source does.
- Set `.font_family(pink::SANS)` (Geist) on the component's root; `pink::MONO`
  for code and figures that need it.
- Elevation is Pink's: `t.menu_shadows()`, `t.tooltip_shadows()`,
  `pink::shadow_sm/md/lg()`. In-flow cards have none. Focus is
  `.focus_visible(move |s| pink::focus_ring(s, ring))` with
  `let ring = t.focus_ring();`.
- Reach for the kit before building a control: `button`, `icon_button`,
  `chip`, `badge`, `segmented`, `toggle_switch`, `checkbox`, `status_pill`,
  `kbd_hint`, `popover_card`, `menu_row`, `menu_heading`, `section_card`,
  `card_row`, `search_input_frame`, `tooltip_card`, `dialog_card` and more.

### Hover, focus and motion

- Hover colors **ease** like Pink's: use `HoverFill` (`.hover_fill`,
  `.hover_fill_fg`, `.hover_fg`, `.hover_border`, `.hover_opacity`) on an
  element with an `.id(…)`, keyed uniquely (`format!("{id}-row-{ix}")`). For
  child colors that follow the row's hover (an icon brightening), read
  `motion::hover_blend(&key, rest, hover)`.
- Use Pink's presets for the job: `HOVER_FADE` for hovers, `menu_in_from` /
  `menu_out` for menus, `DISCLOSURE` for folds, `QUICK` for thumbs and
  chevrons, `STATUS_SWAP` for a changing label, `TOOL_ROW_REVEAL` for rows that
  arrive, `RESORT` for rows that move. Pink never overshoots: no
  `EASE_OUT_BACK`.
- Values that glide when they change (thumb positions, heights, fills):
  `motion::follow(key, target, spec)` / `follow_from(key, from, target, spec)`.
- Loops run on Pink's pulse clock, not `with_animation(..repeat())`: use the
  loaders in `pink::loaders` (`breathing_orb`, `mini_glyph_spinner`,
  `gradient_spinner`, `shimmer_text`, `working_status`…) or
  `motion::pulse_phase` for your own.
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

- **Shadows**: GPUI's `blur_radius` is a Gaussian sigma, which is how Pink
  writes its elevations, so use the kit's shadows as they are.
  `primitives::shadow` converts CSS values instead (CSS blur ÷ 2) and would
  halve Pink's numbers. GPUI paints a shadow list last-on-top and under the
  whole element, so shadows show through translucent fills.
- **Hairlines**: a `0.5px` border can vanish at 1x; Pink's edges are 1px in
  a translucent color (`t.border`, `t.hairline(..)`).
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

Pink's interface icons are Solar Icons (Linear, 24 viewBox, 1.5 stroke).
When Pink has the glyph, copy its SVG from the app's `crates/ui/assets/icons`
into `assets/icons/<slug>/`; otherwise take it from the same set or draw it in
that style. Icons use `stroke="currentColor"` / `fill="currentColor"` and
render with `svg().path(…).size(..).text_color(..)` (GPUI tints SVGs as a
mask), at Pink's sizes (`pink::icon_size`). Multi-color art (file-type icons,
logos) goes through `img("icons/<slug>/logo.svg")`. Credit the source in
`THIRD_PARTY_NOTICES.md`.

## Checking your component

```sh
cargo build && cargo test
cargo clippy --lib --target wasm32-unknown-unknown -- -A clippy::all \
  -D clippy::disallowed_types -D clippy::disallowed_methods
scripts/screenshot.sh <slug> light shots/<slug>.gpui.light.png 894x<stage-height>
scripts/screenshot.sh <slug> dark  shots/<slug>.gpui.dark.png  894x<stage-height>
```

The script renders the gallery stage headlessly (Xvfb + openbox, see the
script). `cargo run -- <slug>` opens the gallery on your component, with a
light/dark switch.

Geometry that Pink scales with its Interface size (hit sizes, the titlebar
cluster, sidebar rows) is checked at sizes 12 and 20: `cargo run -- <slug>
--ui-size 12`, or `play.html?c=<slug>&theme=dark&ui=20` in the browser build.
The story host routes Tab and Shift Tab like Pink's shell, so every tab stop
a component declares is reachable from the keyboard. The Sidebar's shared
columns are measured by `node scripts/site/sidebar-columns.mjs site/dist`
(Playwright), which writes screenshots, measurements and a SHA-256 manifest.

A new component shows up on the showcase site by itself: add it to
`components::COMPONENTS` and the catalog, the player and the site pick it up.
Check it in the browser with `scripts/build-site.sh` and
`python3 -m http.server -d site/dist`, then give it a catalog card picture
with `node scripts/site/thumbnails.mjs site/dist` and commit `site/thumbs/`.

## Before you open a pull request

- Pink's source wins: if the app has the component, the pink-ui one matches it
  value for value, and its doc comments name the Pink code it follows.
- Labels never join facts with `·`, ` -- `, an em or en dash, ` | `, ` / ` or
  a fraction like `2/4`, in visible copy, tooltips, accessibility names,
  descriptions or stories. Use hierarchy, a chip, an icon or a plain phrase
  (`main in my-worktree`, `attempt 2 of 4`). `cargo test` runs
  `tests/label_joiners.rs` over every string literal in `src/` and `web/src/`;
  content that isn't a label (a shell command, a code sample) takes a
  `// joiners: allow <reason>` comment on or above its line.
- Run the review checklist in [`docs/pink-design.md`](docs/pink-design.md)
  over your component: no private palettes, no off-ladder sizes, no shadows on
  in-flow surfaces, no bounce, no saturated fills.
- Write your own demo content and `DESCRIPTION`, and keep the component useful
  on its own: a clean builder API, real callbacks, keyboard support where it
  makes sense.

## Licensing

Only add code, designs, icons or fonts you have the right to publish under MIT,
and record third-party sources and their licenses in `THIRD_PARTY_NOTICES.md`.
