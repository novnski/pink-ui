# Building components

Every component is **one Rust file**, `src/components/<slug>.rs`, the same
way affectUI ships one `.tsx` file per component. A file depends only on GPUI
and the shared modules in this crate (`motion`, `interaction`, `primitives`,
`luna`, `theme`, `story`), and exports `NAME`, `DESCRIPTION` and `story`.
Icons live in `assets/icons/<slug>/`.

There are two kinds (`components::Kind`):

- **Ports** reproduce one of affectUI's free (MIT) components faithfully.
  `src/components/profile_menu.rs` is the reference port: read it first.
- **Originals** are our own components in luna's design language, built
  around an idea from affectUI's Pro catalog. See [Originals](#originals).

The sections below up to Originals apply to ports; the API, state, motion and
GPUI notes apply to both.

## What to reproduce

Each component on affectui.com comes in two **looks** (the style switch on
the site), and each look in light and dark:

| Look      | Web class   | Idea                                               |
| --------- | ----------- | -------------------------------------------------- |
| `Clean`   | `.ui-xxxc`  | One flat card; edges are 0.5px hairline shadows. Site default. |
| `Classic` | `.ui-xxx`   | White cards on a gray tray.                        |

Port both. Share the structure and logic, and switch tokens and sizes on the
look. Port the website demo as the module's `story`, with the same data.

## API shape (luna style)

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
  structs with `new` and setters, mirroring the TypeScript props.

### State

UI state that the web version keeps in `useState` (open/closed, selection,
hover index, typed text, timers) lives in an entity keyed by the component id:

```rust
let state = window.use_keyed_state(
    ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
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
- **Timers and auto-playing demos**: `cx.spawn(async move |this, cx| { loop {
  cx.background_executor().timer(d).await; this.update(cx, |s, cx| …)?; } })`
  in the state constructor, keeping the `Task` in the state so it stops when
  the component unmounts.

### Colors and sizes

- One `Palette` per look × mode, a direct transcription of the CSS custom
  properties (`--ui-xxx-ink: #171717` → `ink: hex(0x171717)`,
  `rgba(0,0,0,.06)` → `black_a(0.06)`). A `Metrics` struct when sizes differ.
- Use the exact px values from the CSS: `px(13.5)`, `.line_height(px(20.))`,
  `.rounded(px(10.))`. Font weights: 400 `NORMAL`, 500 `MEDIUM`,
  600 `SEMIBOLD`, 650 → `FontWeight(650.)`, 700 `BOLD`.
- `font-family: Inter` is already set by the story wrapper and the gallery
  (`primitives::SANS`); use `primitives::MONO` for monospace.
- `box-shadow` → `.shadow(vec![shadow(x, y, blur, spread, color), …])`;
  `0 0 0 1px c` → `ring(1., c)`. Inset shadows don't exist: use a border.
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
- Curves: `motion::EASE_OUT_QUINT` is affectUI's `cubic-bezier(.22, 1, .36, 1)`;
  `EASE`, `EASE_OUT`, `EASE_IN_OUT` match CSS keywords.
- Hosts keep fades and followers moving: the story wrapper adds
  `motion::driver()` (an invisible element that requests frames after
  layout). A view that renders its own animated content calls
  `motion::keep_alive(window)` at the end of its render.

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

### Icons and images

Copy each inline `<svg>` into `assets/icons/<slug>/<name>.svg` with
`xmlns="http://www.w3.org/2000/svg"`, `stroke="currentColor"` /
`fill="currentColor"`, and render it with `svg().path(…).size(..).text_color(..)`
(GPUI tints SVGs as a mask). Strokes with `vector-effect: non-scaling-stroke`
need their width scaled by `viewBox / rendered size` (1.1px at 16px on a
24 viewBox → `stroke-width="1.65"`); keep such variants in a `clean/`
subfolder. Multi-color logos go through `img("icons/<slug>/logo.svg")`.

## Checking your port

```sh
cargo build && cargo test
scripts/screenshot.sh <slug> light shots/<slug>.gpui.light.png 894x<stage-height> clean
scripts/screenshot.sh <slug> dark  shots/<slug>.gpui.dark.png  894x<stage-height> classic
```

The script renders the gallery stage headlessly (Xvfb + openbox, see the
script) at the website's stage size; compare it side by side with the site.
`cargo run -- <slug>` opens the gallery on your component, with switches for
light/dark and Clean/Classic.

## Originals

An original takes the *idea* of a Pro component (what it is for, how it
behaves, how its motion feels) and builds our own component around it. It is
not a port: never transcribe a Pro component's code, CSS values, color tokens,
sizes, layout structure, copy, demo data or SVG paths.

- Studying the Pro demo (its behavior, CSS and screenshots) to understand the
  interaction and the motion – what animates, in which order, roughly how long
  and with which kind of easing – is fine. Then close it and design your own.
- Style with [`crate::luna`](src/luna.rs): `Luna::get(cx)` tokens (surfaces,
  text, borders, fills, accent, status colors), the radius and spacing
  ladders, `luna::SANS` (Geist) / `luna::MONO`, and the control kit
  (`button`, `icon_button`, `chip`, `badge`, `kbd`, `card`, `overlay`).
  Originals follow light/dark; the Clean/Classic look doesn't apply.
- Use luna's motion presets (`motion::MENU_IN`, `GLIDE`, `HOVER_FADE`, …)
  and `HoverFill` for hovers.
- Write your own demo content and your own `DESCRIPTION`; draw your own
  icons (simple 24×24 strokes, `stroke-width="1.5"`).
- Keep the component useful on its own: a clean builder API, real callbacks,
  keyboard support where it makes sense.

## Licensing

Ports keep affectUI's MIT attribution in `THIRD_PARTY_NOTICES.md`. affectUI's
Pro components are commercially licensed and their terms forbid redistributing
them in a component library, which is why the Pro catalog only ever supplies
ideas here, never code or design.
