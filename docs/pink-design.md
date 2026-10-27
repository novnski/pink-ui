# Pink design language

This guide is for anyone writing a Pink component (`Kind::Pink`), whether a person or an agent.
Follow it and the component will look like it belongs in the PINK desktop app. The kit in
`src/pink.rs`, `src/pink/{controls,surfaces,loaders}.rs`, `src/motion.rs` and `src/interaction.rs`
reproduces Pink's own UI crate (`pink/crates/ui/src`) value for value. Build from the kit. Don't
re-derive colors or sizes. If this guide and Pink's source disagree, Pink's source wins. Fix the
kit and this guide to match it.

## 1. The idea

PINK is a calm, dense tool. Surfaces are neutral and close in tone: near-black panels in dark mode,
cool off-whites in light mode. They are separated by hairlines and faint, text-tinted washes, not
by color or depth. Content sits flat in the flow and is never shadowed. Only floating things
(menus, tooltips, dialogs, the palette) get a hairline and elevation. The accent (Violet) is
reserved for focus, selection and live state: the focus ring, selection rings, the caret, the
working orb, unread dots and switch tracks. It never decorates. Type is Geist at 11–14 px with
weights 400/500/600. Motion is short and decelerating, and it never bounces.

The kit reproduces what users see: Pink's default **Frosted** treatment, with the default theme
(`pink-dark` / `pink-light`) and the default Violet accent, using the runtime palette Pink paints
after `Theme::from_variant` and its contrast hardening. Under frost the shell is a glass tint over
the desktop, and popovers, menus, dialogs, the palette and the composer are a 15% tint over a
16 px backdrop blur of the window, so they read as dark (or light) glass, not grey slabs.

pink-ui's GPUI (`gpui-pre 0.3.9`) has no backdrop blur and no `EdgeFade`, and a translucent tint
without blur would show the content behind it unblurred. So the kit adapts frost in two halves:

- **Text is Pink's frosted text.** Every secondary tone (`text_muted`, `text_faint`,
  `border_strong`, the `*_muted` status copy, `code_text`, syntax) and every paint-time floor
  (`readable_*`, `legible`, `focus_ring`, `for_popup`) is derived with Pink's own algorithm over
  Pink's frosted reading surfaces: the glass over the *adverse* desktop (white under dark, black
  under light) and the floating, input and card tints over it. That is why dark `text_muted` is
  `#c9c9cc`: the frosted chrome can be as light as `#3d3d3d`.
- **Plates are Pink's frosted tints, baked over the canvas.** Each tint is flattened over
  `Pink::bg`, the canvas behind every demo, which stands in for the blurred backdrop. Every
  helper returns an opaque color: a popover plate equals the canvas (as a frosted card over a dark
  window does), and floating cards separate from it by their hairline, `menu_shadows` /
  `shadow_lg` and Pink's frost elevation rings (`t.elevation_rings()`). Tooltips and toasts are
  opaque in Pink too, and keep Pink's exact `tooltip_surface()`.

Paint plates from the helpers below, never from `surface`, `surface_overlay` or `input_bg`
directly, and don't fake frost with alpha.

### Ground rules

1. Start every render with `let t = Pink::get(cx);`. It also records the mode `pink::glyph()` reads.
2. Sizes are px at Pink's default 16 px interface size (`ui_rems(13.)` paints 13 px).
3. Set `.font_family(pink::SANS)` on the component root. The gallery's story wrapper sets Geist too, but a
   component dropped into another app only gets it from its own root.
4. Sizes come from a ladder (§3), colors from a token or helper (§2), durations from `motion` (§5).
5. Use `web_time::Instant` for clocks, never `std::time`.

## 2. Tokens

`Pink::get(cx)` returns the active mode's tokens. `Pink::dark()` / `Pink::light()` /
`Pink::for_mode(bool)` return a fixed mode. Alphas are given as decimals.

| Role | Field / helper | Dark | Light |
|---|---|---|---|
| Content panel, window, the canvas behind demos | `bg` | `#060606` | `#fbfcfe` |
| Shell chrome, sidebar (glass `surface` .80 / .94 over `bg`) | `chrome_surface()` | `#0c0c0c` | `#f1f4f7` |
| Settings section cards (`surface` .40 over `bg`) | `card_glass_bg()` | `#090909` | `#f7f8fb` |
| Popover / menu / dialog / palette / composer plate (15% tint over `bg`) | `surface_bg()` | `#060606` | `#fafcfe` |
| Input plate (`input_bg` .52 / white .30 over `bg`) | `input_glass_bg()` | `#1e1e20` | `#fcfdfe` |
| Tooltip, hover card, toast (opaque in Pink) | `tooltip_surface()` | `#262627` | `#fbfbfc` |
| Inline standalone card | `surface_card` | `#0e0e0e` | `#ffffff` |
| Opaque raised disc | `surface_raised` | `#343438` | `#e7ecf2` |
| Primary text | `text` | `#e8e8ea` | `#202631` |
| Secondary chrome text | `text_muted` (= `text_dim`) | `#c9c9cc` | `#445064` |
| Hints in chrome | `text_faint` | `#ccccce` | `#4d586a` |
| Secondary / tertiary on the canvas | `content_muted` / `content_faint` | `#a9a9ae` / `#8f8f94` | `#445064` / `#586579` |
| Composer placeholder | `placeholder` | `#949499` | `#586579` |
| Muted / faint text inside a menu | `t.for_popup().text_muted` / `.text_faint` | `text` .64 / .584 | `text` .64 / .688 |
| Card and popover edge | `border` | white .102 | black .122 |
| Tooltip and capsule edge | `border_strong` | white .508 | black .454 |
| Divider / strong divider | `separator(false)` / `separator(true)` | `#eaeaec` .08 / .14 | `#14181f` .108 / .189 |
| Chrome hover wash, trigger chips | `element_hover` | white .110 | black .059 |
| Accent (also `accent_strong`, `busy`, `caret`) | `accent` | `#8b7cf6` | `#5b43e8` |
| Selected chip plate, inline code plate | `accent_wash` (= `code_wash`) | accent .220 | accent .122 |
| Focus ring | `focus_ring()` | `#bbb3fa` | `#5b43e8` |
| Text selection | `selection` | accent .349 | accent .239 |
| Solid plate / its label | `solid` / `on_solid` | `#ebebef` / `#000` | `#232328` / `#fff` |
| Danger / copy | `danger` (= `danger_strong`) / `danger_muted` | `#f87171` / `#f8bbbc` | `#dc2626` / `#a72629` |
| Warning / copy | `warning` / `warning_muted` | `#facc15` / `#f6d34a` | `#a16207` / `#7c5011` |
| Success / copy | `success` / `success_muted` | `#34d399` / `#75ddb7` | `#15803d` / `#176638` |
| Inline code text | `code_text` | `#ada2f9` | `#5b43e8` |
| Spinner tints `[light, mid, deep]` | `glyph` | `#aba1f9 #8b7cf6 #7266ca` | `#7864ec #5b43e8 #4332ac` |

Code and terminal tokens are `code_text`, `diff_add`, `diff_del`, `diff_hunk_bg`, `terminal_*`,
`t.syntax()` and `t.ansi()`.

### Which helper to use

| Need | Use | Not |
|---|---|---|
| Descriptions, secondary labels | `t.readable_muted(0.85)` | raw `text_muted` with an alpha |
| Ghost labels, default glyph tone | `t.readable_muted(0.95)` = `t.glyph_tone()` | |
| Hints, section labels in lists | `t.readable_faint(0.9)` | |
| Status copy (errors, warnings) | `t.readable_tone(t.danger_muted, 0.9)` | raw `danger` text |
| Text on its own translucent pill | `t.readable_on(tone, tone.opacity(0.12), pink::TEXT_CONTRAST_TARGET)` | |
| Status glyph | `t.readable_icon(tone)` (floored at 3.1:1) | |
| Muted text or glyphs inside a menu or popover | `let p = t.for_popup();` then `p.text_muted` / `p.text_faint` | the chrome `text_muted` |
| A tone on the composer's controls row | `t.readable_on_composer(tone, pink::TEXT_CONTRAST_TARGET)` | `readable_on(tone, t.surface_bg(), ..)` |
| The composer pill (and the question panel sharing it) | `.bg(t.surface_bg())` with a 1 px `border` | `input_glass_bg()` (an input plate) |
| Resting plate (ghost/secondary buttons, wells) | `t.fill(FillTier::Quaternary)` | `ink(0.04)` |
| Hover | `t.fill(FillTier::Tertiary)`, through `hover_wash` | GPUI `.hover()` (it snaps) |
| Pressed, or hover on a filled control | `t.fill(FillTier::Secondary)` | |
| Selected chrome row (sidebar, nav) | `t.fill(FillTier::Primary)` = `glass_selected_bg()` | `accent_wash` |
| Selected row in a floating card | `card_selected_bg()` + `.shadow(t.card_selected_shadows())` | |
| Menu row hover | `menu_hover_bg()` (60% of the selected wash) | |
| Custom plate / state wash / hairline | `t.ink(a)` (code, chips) / `t.wash(a)` (bubbles) / `t.hairline(a)` (light ×1.35) | white/black literals, `t.border.opacity(..)` |
| Scrim | `t.scrim(pink::MODAL_SCRIM)` (or `pink::modal_scrim`) | |
| Focus | `let ring = t.focus_ring(); el.focus_visible(move \|s\| pink::focus_ring(s, ring))` | a border |
| Label on Primary / Danger plate | `t.on_solid` / `t.on_danger()` | `on_accent` |

`readable_*` take a dark-mode opacity. Dark paints the token at that opacity; light maps the
opacity onto a contrast between the token's own and the 4.6:1 floor (1 keeps the token, 0.5 or less
sits on the floor), so a quieter label stays visibly quieter instead of every request landing on
one grey (Pink `Theme::light_tier`, UI rethink bug 18). Both are floored to 4.6:1 (text) or 3.1:1
(icons) over Pink's frosted reading surfaces. They are memoized, so call them on every frame. Fill
tiers are tinted by the text hue, never pure white or black.

Text contrast tiers (Pink UI rethink D38): primary about 12:1 (`TEXT_CONTRAST_PRIMARY`), secondary
about 6:1 (`TEXT_CONTRAST_SECONDARY`), tertiary on the 4.6 target (`TEXT_CONTRAST_TERTIARY`).
Nothing that is text, placeholders and metadata included, goes below 4.5:1. On the light sidebar
chrome the kit measures 12.0 for `text`, 6.5 for `readable_muted(0.85)` and 5.5 for
`readable_faint(0.6)`.

## 3. Ladders

Pink's UI rethink (milestone 0) names its foundation by role: type roles, spacing roles, radius
roles, contrast tiers and hit sizes. New and migrated surfaces pick a role; the size names below
stay for older call sites, as they do in Pink.

### Type roles (`TypeRole`, apply with `.type_role(..)`; Pink `typography::TypeRole`)

| Role | Size / line | Use |
|---|---|---|
| `Chrome` | 13 / 18 | titlebar, toolbar, menu and row titles |
| `ToolActivity` | 14 / 20 | tool activity sentences in the transcript |
| `ComposerInput` | 14 / 20 | the composer's input |
| `Metadata` | 12 / 16 | timestamps, counts, secondary row lines |
| `KeyHint` | 11 / 14 | keyboard shortcut hints only |
| `Prose` | 15 / 24 | transcript prose |

A role sets size and line height; weight stays with the caller. Hierarchy comes from size and
colour tier together, never from size alone.

### Type (`TextStyle`, apply with `.type_style(..)`; `use crate::pink::TypeStyleExt`)

| Step | Size / line | Weight | Role |
|---|---|---|---|
| `Caption2` | 10 / 13 | 400 | counts, uppercase badges |
| `Caption` | 11 / 14 | 400 | meta, chips, timestamps, tooltips, menu headings (600) |
| `Footnote` | 12 / 16 | 400 | descriptions, tool rows, secondary rows, Md buttons |
| `Body` | 13 / 18 | 400 | default UI text, menu rows, row titles (500) |
| `Prose` | 14 / 21 | 400 | chat prose, text inputs |
| `Title3` | 15 / 20 | 600 | card and dialog titles, metric values |
| `Title2` | 17 / 22 | 600 | section titles |
| `Title1` | 20 / 25 | 600 | page titles |
| `Display` | 26 / 32 | 700 | empty-state hero only |

The workhorses are 13 for primary rows, 12 for secondary rows and 11 for meta. 500 is for row
titles, buttons, chips and status words. 600 is for titles, field labels and menu headings. There
is no 16 px step. Code is `text::CODE` 12.5 in `pink::MONO` (Geist Mono; pink-ui bundles only its
400/500 weights). Use `pink::tabular_figures()` for anything that ticks.

### Radius by role (`pink::radius`, px)

Roles (Pink `theme::radius`, UI rethink D37): `CONTROL` 6 controls and rows, `CODE` 8 code blocks,
notices and tooltips, `POPOVER` 12 popovers, menus and the content frame, `DIALOG` 16 dialogs and
sheets, `COMPOSER` 20 the composer plate role. Pink's composer itself still draws its pill at 26
(`composer::COMPOSER_RADIUS`), which the kit keeps as `COMPOSER_PILL` for the composer and the cards
styled after it.

| px | Const | Use |
|---|---|---|
| 4 | `XS`, `chrome::RADIUS_S` | checkboxes, status pills, 20 px row actions |
| 5 | `KBD` | kbd hints, key caps |
| 6 | `SM`, `CONTROL` | Sm controls, toolbar inputs, tool header plates, tab close |
| 7 | `TILE` | 28 px identity tiles |
| 8 | `MD`, `CODE`, `MENU_ITEM`, `TOOLTIP`, `PALETTE_ITEM` | Md/Lg controls, menu rows, fields, tooltips, message badges |
| 10 | `LG` = `PANEL` | code surfaces, transcript notices, tool chips |
| 12 | `POPOVER`, `SECTION_CARD`, `STRIP` | popovers, settings cards, strips, `card()` |
| 14 / 18 / 20 | `STACK_NOTICE` `STACK_POPUP_ROW` / `STACK_TRAY` / `STACK_POPUP` | the stack above the composer |
| 16 | `XL`, `DIALOG`, `PALETTE`, `BUBBLE` | dialogs, command palette, user bubble |
| 20 | `COMPOSER` | the composer plate role |
| 26 | `COMPOSER_PILL` | the composer pill as Pink draws it, composer-styled cards |
| full | `rounded_full()` | trigger chips, badges, switches, notice actions, toasts |

Nested corners: `radius::inner(outer, inset)` = `max(outer − inset, 4)` (12 px card, 4 inset → 8).

### Spacing (`pink::space::S2..S32` = 2 4 6 8 10 12 16 20 24 32; `pink::layout`)

Roles (Pink `theme::space`, UI rethink D33), a 4 pt structure with 2 pt optical nudges:
`OPTICAL` 2, `ICON_LABEL` 6, `SIBLING` 8, `CONTROL_PAD` 10, `PANEL_INSET` 16, `SECTION` 24.

| Context | Value |
|---|---|
| Icon → label in a control | 4 (Sm) / 6 (Md, Lg) |
| Menu row icon → label | 10 |
| Title ↔ description stack | 2 (`row_text`), 1 (`space::TEXT_STACK_GAP`) |
| Between menu rows / card inset | 2 (`layout::MENU_GAP`) / 4 (`layout::CARD_INSET`) |
| Row padding | menu `px 8 / py 6`; settings `px 16 / py 12`, min 48 |
| Card padding | 4 menus, 10–12 notices and cells, 16 settings rows, 20 dialogs; rarely more |
| Between settings sections | 24 (`layout::SECTION_GAP`) |
| Trigger → popover / window margin | 6 (`POPOVER_GAP`) / 8 (`WINDOW_MARGIN`) |

### Icons and controls

| Ladder | Values |
|---|---|
| `icon_size` | `XS 12` status glyphs by caption text · `SM 14` toolbars, chips, chevrons, tool rows · `MD 16` buttons, menu icons, tiles · `LG 20` titlebar, empty states |
| `ControlSize` | `Sm` h24 r6 11/14 pad 8 glyph 12 gap 4 (toolbars, row actions, chips) · `Md` h28 r8 12/16 pad 10 glyph 14 gap 6 (settings column, panel headers) · `Lg` h32 r8 13/18 pad 12 glyph 16 gap 6 (dialogs, sidebar footer) |
| `chrome` | controls `CONTROL_S/M/L` 20/28/32, icons 12/16/18, radii 4/6/8 |
| Hit sizes | `chrome::MIN_HIT` 24: no clickable target is smaller, at any Interface size. `CONTROL_S` is a visual size; give the control a `MIN_HIT` box (Pink's per-project New thread button), and offset an absolutely placed one by `chrome::hit_outset(visual)`. Controls are 24, 28 or 32 tall, 28 by default |
| Interface size | `pink::ui_scale(cx)` (Interface size over 16; the gallery's `--ui-size 12`, the player's `&ui=12`). `chrome::scaled_control(step, scale)` clamps to 24 to 32, `chrome::scaled_icon` to 12 to 18 |
| Titlebar cluster | `layout::titlebar_control_size(scale)` (24 at size 12, 28 at 16, 32 at 20), `layout::cluster_buttons_width(scale)` (82 / 94 / 106), group gap 8, control gap 2, `TITLEBAR_IDENTITY_GAP` 12 before the title. The controls take Tab and show the focus ring |
| Fixed heights | status pill 16 · key cap 22 · notice action 24 · message badge ≥ 24 · trigger chip 28 · toolbar input 28 · switch 34×20 · checkbox 16 · avatar 20 |

## 4. Recipes

Every builder returns a `Div` or `Stateful<Div>`, so keep chaining. The Pink symbol named in each
recipe's heading or last line is the original to check against.

### In-flow cards, never shadowed (Pink `transcript::code_surface_frame`)

| Card | Kit | Anatomy |
|---|---|---|
| General card on the panel | `pink::card(&t)` | r12, 1 px `border`, `card_glass_bg()` |
| Code / output surface | `pink::code_surface(&t)` | r10, `border`, `ink(0.035)`; header strip 28 tall, `ink(0.02)`, `border_b`, 11/500 `content_muted` |
| Notice in content | `div().rounded(px(radius::LG)).bg(tone.opacity(0.08)).border_1().border_color(tone.opacity(0.24))` | `px 12 / py 10`, glyph 14 `readable_tone`, title 13/600, detail 12/17 `content_muted` |
| Chip card (tool, subagent) | `div().h(px(30.)).rounded(px(10.)).bg(t.ink(0.03)).border_1().border_color(t.hairline(0.07))` | `px 8` |
| Standalone auth / empty card | `pink::inline_card(&t)` | the one in-flow card with `shadow_lg` |

### Floating popover / menu

```rust
motion::menu_in_from("model-menu", motion::MENU_RISE_DOWN,   // +6 (MENU_RISE_UP) when above
    pink::popover_card(&t).flex().flex_col().w(px(pink::layout::MENU_W_M))
        .child(pink::menu_heading("Recent", &t))
        .child(/* menu rows */)
        .child(pink::menu_separator(&t)))
```

`popover_card` is r12, `border`, `surface_bg()`, the frost `elevation_rings()` over
`menu_shadows()`, `p 4`, gap 2, clipped, 13 Geist. Its headings and kbd hints read `for_popup()`. Widths `MENU_W_S/M/L` (240/280/320) may widen with content, never shrink. Place it with
`anchored().snap_to_window_with_margin(px(8.))`, 6 px from the trigger. Leading-aligned menus shift
−4 so row text lines up with the trigger glyph, and submenus sit beside the parent at −(4 + 6).
Use `popover_card_flush` for rail + list panes and `stack_popup_card` (r20, p6) above a composer.
Pink: `popover::popover_card`.

### Menu row

| Row | Kit | Notes |
|---|---|---|
| Pointer menu | `pink::menu_row(id, active, &t)` | gap 10, `px 8 / py 6`, r8, 13; rest `text@.9` → `text`, wash → `menu_hover_bg()`; active `card_selected_bg()` |
| Keyboard menu | `pink::menu_row_nav(selected, highlighted, &t)` | one shared highlight for keys and pointer (move it on `.on_mouse_move`); 14 px check in `text` in the 28 px column |
| Completions | `pink::menu_row_cursor(highlighted, &t)` | no check column, r14 |
| Leading icon | `pink::line_glyph(glyphs::PLUS, 16., t.text_muted)` or a 16 px `svg()` in `text_muted` | |
| Trailing | `pink::kbd_hint("⌘N", &t)`, or `pink::menu_chevron(&t)` after a `flex_1` label | |
| Destructive | `.text_color(t.readable_tone(t.danger, 1.0))`, icon `t.readable_icon(t.danger)` | |
| Persistent choice (picker) | add `.shadow(t.card_selected_shadows())` to the selected row | 1 px inset ring |
| Sections / empty | `menu_separator` (full bleed), `menu_section`, `menu_empty("No matches", &t)` | |

Hover never looks like selection. Pink: `popover::menu_row`.

### Section headers and labels (Pink `popover::menu_heading`)

| Where | Recipe |
|---|---|
| Inside a menu | `pink::menu_heading("Recent", &t)`: 11/600 `for_popup().text_muted`, UPPERCASE with hair-space tracking. This is the only uppercase label. |
| List / sidebar group | h28, `px 8`, r6, `hover_wash`; 11/600 `readable_faint(0.9)`, sentence case; chevron only on group hover; trailing count 11 `readable_faint(0.7)` |
| Palette group / small group label | 11/500 sentence case: `text_faint` (`px 10 / pb 4`, after a `border@0.6` rule) / `readable_muted(0.6)` (`px 8 / pb 2`) |
| Settings card header | `pink::section_header(title, Some(desc), &t)`: Title3 + 12/16 description |
| Form group label | `pink::field_label("Model", &t)`: 13/600 `readable_muted(0.85)` |
| Page header | Title1 + a 13 count in `readable_muted(0.7)`; subtitle `mt 6`, 13/19 `readable_muted(0.9)`, max width 600 |

### Settings-style section card and row

```rust
pink::section_card(&t)                                   // mt 24, r12, border, clipped column
    .child(pink::section_header("Notifications", Some("How PINK reaches you.".into()), &t))
    .child(pink::card_row(false, &t)                     // min 48, px 16 / py 12, top hairline
        .child(pink::row_tile("icons/<slug>/bell.svg", &t))
        .child(pink::row_text("Sounds", Some("A chime when a turn ends.".into()), &t))
        .child(pink::row_controls().child(pink::switch_control("sounds", "Sounds", on, &t)
            .on_click(toggle))))
```

Put chip groups and lists under the text in `pink::row_block()`, and fine print after the card in
`pink::section_footer(copy, &t)`. A page column is max 720 wide with `px 32 / pt 28 / pb 72`.
Rows have no hover, because rows aren't controls. Pink: `settings::widgets::section_card`.

### Search field and inputs

| Field | Kit / recipe |
|---|---|
| Search at the top of a picker | `pink::search_input_frame(&t)`: borderless `fill(Quaternary)`, r8, `px 10 / py 6`, 13, no icon |
| Search strip (model picker) | h40, `px 10`, gap 8, `magnifer` 14 `text_muted`, 13 input, `border_b hairline(0.08)` |
| Palette search | min-h 44, `px 16 / py 8`, gap 10, 16 icon `text_muted`, 14 input, trailing `kbd_hint("⌘K")`, `border_b hairline(0.06)` |
| Dialog / form field | `pink::dialog_field(&t)` (14) · compact editor `pink::input_frame(&t)` (13) |
| Toolbar field | `pink::toolbar_input(&t)`: h28 r6 `ink(0.035)` 12 |
| The editor inside | `TextInput::new(window, cx).placeholder("Search").placeholder_color(t.text_faint).caret_color(t.caret).selection_color(t.selection)` |
| Match highlight | text in `code_text` over an r3 `code_wash` underlay, with no weight change |

Frames don't change on focus. The 2 px accent caret, blinking every `layout::CARET_BLINK_MS`, shows
it. Pink: `popover::search_input_frame`.

### Chips, pills, badges, kbd (Pink `settings::widgets::badge`)

| Element | Kit | Notes |
|---|---|---|
| Multi-choice chip | `pink::chip(id, selected, ControlSize::Sm, &t)` | selected: `accent_wash`, edge `accent@.55`, `readable_tone(accent)`, 500 |
| 2–5 fixed options | `pink::segmented(id, vec![Segment::new("a", "All")], ix, width, size, &t, on_select)` | thumb slides on `QUICK` |
| Ghost pill (model, branch) | `pink::trigger_chip(id, set, open, &t)` | h28 full, 12/500, no border, no caret |
| Settings status | `pink::badge("Active", Tone::Success, &t)` | Neutral outlines in `border`; Warning/Danger outline at .35 |
| Waiting/failed pill | `pink::status_pill("Approve", t.warning, None, &t)` | h16 r4 `tone@.12`, 11/14 500 floored on its fill |
| Message badge | `pink::message_badge(&t)` | ≥ 24, r8, `ink(0.06)`, 12/500 `text_muted` |
| "New" / "Default" markers | `div().h(px(16.)).px(px(5.)).rounded(px(4.)).bg(t.accent.opacity(0.14))` / bare text | 10.5/600 `readable_tone(accent, 1.0)` / 11/600 `text_muted`, no plate |
| Accelerator | `pink::kbd_hint("⌘N", &t)` | `px 5 / py 1`, r5, `fill(Quaternary)`, 11 mono |
| Footer legend | `key_hint(icon, "Select", &t)`, `key_hint_text("esc", "Close", &t)`, `key_hint_pair("↑", "↓", "Navigate", &t)` | |
| Capsule action in a notice | `pink::notice_action(id, &t)` | h24 full, `border_strong` edge |
| Identity | `pink::avatar("L", &t)`, `pink::monogram_tile(pink::project_tint(ix), name, 20., &t)` | |
| Option card | `pink::selection_ring(selected, 6., &t, preview)` | 2 px accent ring 2 px outside |

### Buttons

| Kind | Use |
|---|---|
| `ButtonKind::Primary` | the one solid action per group: a `text`-colored plate, `on_solid` label |
| `ButtonKind::Secondary` | filled, outlined push button (`separator(true)` edge) |
| `ButtonKind::Ghost` | quiet actions, Cancel, Back: `readable_muted(0.95)` → `text` |
| `ButtonKind::Danger` | destructive confirmation: `danger_strong` plate, `on_danger()` label |
| `ButtonKind::Accent` | legacy: paints as Primary, so don't use it in new code |
| `button_enabled(.., false, ..)` | disabled: 0.45 opacity, no hover, pointer or tab stop |
| `pink::icon_button(id, path, size, &t)` | glyph-only; add an accessible name and a tooltip |
| `pink::select(id, label, size, open, &t)` | pop-up button showing the current choice |
| Chrome / row icon buttons | titlebar 28 r6 `glass_hover`, 16 `text_muted`; sidebar row actions 20 r4 `hover_wash`, 12 |

```rust
pink::button("save", ButtonKind::Primary, ControlSize::Md, &t)
    .child(pink::glyph("icons/<slug>/check.svg", ControlSize::Md).text_color(t.on_solid))
    .child("Save")
    .on_click(|_, window, cx| { /* ... */ })
```

There is no press state, because activation is the click. Pink's button has no loading state
either. If you need one, put `mini_mono_spinner(key, 2., t.on_solid)` where the glyph goes. Pink:
`button::button`.

### Status

| State | Signal | Kit |
|---|---|---|
| Running item (row, tab, chip) | 2×3 glyph spinner | `loaders::mini_glyph_spinner(format!("{key}-spin"), 2.0, t.glyph)` |
| Live, presence, working line | breathing orb | `loaders::breathing_orb(t.accent)` |
| Neutral busy ("Reconnecting…") | mono spinner | `loaders::mini_mono_spinner(key, 2.0, t.text_muted)` |
| Waiting on the user | the glyph it waits for (shield, question bubble) in `readable_icon(t.warning)`, or `status_pill` | |
| Failed | one red glyph (`danger-circle`) + one red word, 11/500 `readable_tone(danger, 1.0)` | never a red row; group headers stay quiet |
| Done, unseen | 6 px `accent` dot | `div().size(px(6.)).rounded_full().bg(t.accent)` |
| Dot colors (sidebar) | Working `busy@.55`, Approval `warning@.72`, Input `accent@.6`, Error `danger@.65`, Done `success@.9`, Idle `ink(0.14)` | below full strength on purpose |
| Meter | 6 px capsule on `fill(Secondary)`; fill `text_muted`, `warning` ≥ 75%, `danger` ≥ 90% | no accent meters |

Shape comes before color: every state has its own shape at ≥ 3:1. Done states are quiet, with no
green checks and no success pops. Pink: `loaders::mini_glyph_spinner`.

### Numbers, timers, mono (Pink `typography::tabular_figures`)

- Ticking numbers (timers, time-ago, counts): sans 11, `.font_features(pink::tabular_figures())`,
  `content_faint`, formatted by `loaders::format_elapsed(secs)` (`12s`, `1m 5s`). The working line
  adds a `· ` prefix: `loaders::working_timer(secs, t.content_faint)`.
- Metric value: Title3 over an 11/14 label. Thousands separators (`1,234`); right-aligned in tables.
- Mono is for machine text only: paths (12 `text_dim`), `+N`/`−N` counts (11, `readable_tone`),
  hunk headers, kbd hints and jump hints. Timers stay in Geist.

### Hover-revealed actions

| Pattern | Mechanism |
|---|---|
| Plate and label brighten together | `.hover_fill_fg(key, rest, hover, rest_fg, hover_fg)` (150 ms) |
| Reveal on row hover | `.opacity(0.).group_hover(group, \|s\| s.opacity(1.))` plus `.focus_visible(..)` showing it too |
| Always discoverable | trailing tile at `opacity 0.5 → 1` on row hover |
| Content swap | sidebar corner: time-ago ↔ actions on row hover |
| Reserved lane | reserve the lane's height (transcript meta lane: 28) so revealing never moves layout |
| Text brighten | `motion::hover_blend(&row_key, t.content_muted, t.text)` on children of the hovered row |

One hover key per element, unique on screen (prefix it with the component slug). Pink:
`interaction::HoverFill::hover_wash`.

### Empty, loading, error (Pink `surface_chrome::surface_state`)

| State | Kit / recipe |
|---|---|
| Empty or error pane | `pink::surface_state(icon_28_text_faint, "No runs yet", Some(body), &t)`, an action at `mt 6` |
| Empty menu | `pink::menu_empty("No matches", &t)` |
| No search results | 13 "No results" + a muted hint, `py 24 / px 16`, gap 6 |
| Loading pane | `loaders::gradient_spinner(3.0, t.glyph)` above a 12 `readable_muted(0.7)` label, column gap 8. Show a loader instead of a blank pane. |
| Loading list / menu | `loaders::skeleton_rows(n, t.fill(FillTier::Quaternary))`, `skeleton_menu_rows(n, ..)`, label ghost `skeleton_bar(w, t.fill(FillTier::Tertiary))` |
| Inline error | `p 8`, gap 6, 12 `readable_tone(danger, 1.0)`; retry is a `Secondary` `Sm` button |
| Settings error/warning strip | `let (plate, edge, copy) = pink::strip_colors(t.danger, t.danger_muted, &t);` r12 `px 16 / py 12`, 12.5, icon 16 |
| Toast | h34 full, `hairline(0.10)`, `tooltip_surface()`, `menu_shadows()`, `pl 12 / pr 4`, gap 8, 12.5; enters on `TOAST_IN` + 8 px rise |

### Tooltips

```rust
el.tooltip(pink::text_tooltip("Copy")).tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
```

`text_tooltip` is `tooltip_card` (r8, `border_strong`, opaque `tooltip_surface()`,
`tooltip_shadows()`, Geist) at `px 8 / py 5` in Caption: one line up to 48 characters, else wrapped
at 260. A control tooltip is `tooltip_card` at `px 10 / py 7`, gap 2, 12: a 500 `text` title with a
`kbd_hint`, then 11 `text_muted` detail lines. Near the window bottom, show it 7 px above the
trigger. Pink: `popover::tooltip_card`.

### Dialogs

```rust
pink::modal_scrim(&t).child(motion::dialog_in("delete-dialog", pink::dialog_card(&t)
    .child(pink::dialog_title("Delete this chat?", &t))
    .child(pink::dialog_body("It can't be restored.", &t).mt(px(6.)))
    .child(div().mt(px(16.)).flex().justify_end().gap(px(8.))
        .child(pink::button("cancel", ButtonKind::Ghost, ControlSize::Lg, &t).child("Cancel"))
        .child(pink::button("delete", ButtonKind::Danger, ControlSize::Lg, &t).child("Delete")))))
```

`dialog_card` is 360 wide, `p 20`, r16, `surface_bg()`, a `separator(false)` edge, and the frost
`elevation_rings()` over `shadow_lg`. Use `pink::checkbox(id, "Don't show again", checked, &t)` for an opt-out. A command
palette is `pink::palette_card(560., radius::PALETTE, &t)`, anchored at `max(20% of the viewport,
88)` from the top, entering on `PALETTE_IN` + 6. Its header and footer rules are `hairline(0.06)`,
and its footer holds `key_hint`s. Pink: `popover::dialog_card`.

## 5. Motion

`crate::motion` is Pink's catalog. Pick from it. Don't invent durations.

| Job | Preset / helper |
|---|---|
| Hover wash, label brighten | `HoverFill` (`HOVER_FADE`: 150 ms Tailwind `(.4,0,.2,1)`) |
| Press, tooltip fades | `MICRO` (120 ms standard) |
| Switch track, segment thumb, selection ring, stand-alone chevron | `QUICK` (160 ms) through `motion::follow(key, target, QUICK)` |
| Menu in | `menu_in_from(id, MENU_RISE_DOWN \| MENU_RISE_UP, el)`: `MENU_IN` 180 ms quint, ±6 from the trigger side |
| Menu out | `menu_out(format!("{id}-out"), motion::menu_exit_progress(closed_at), el)`: `MENU_OUT` 120 ms, opacity only; occlude the dying menu |
| Dialog in | `dialog_in(id, el)`: 180 ms ease + 2 px rise |
| Palette / toast in | `rise_in(id, PALETTE_IN, PALETTE_IN_RISE, el)` / `rise_in(id, TOAST_IN, TOAST_IN_RISE, el)` (6 / 8 px) |
| Disclosure / fold, switch knob | `DISCLOSURE` 300 ms `(.32,.72,0,1)`; `fold_progress(open, elapsed)` gives height and body alpha; the body fades in on `TOOL_BODY_FADE` + 6 px and is gone by 60% of a close; the fold's chevron rotates 0 → 90° with it |
| Row reveal (new rows) | `TOOL_ROW_REVEAL` 360 ms expo height grow; `tool_row_arrival_delay(ix, new_group)` = 90 ms, then 65 ms per row; content rises 4 |
| View entrance | `fade_in_rise(id, el)` (`FADE_IN` 500 ms expo + 4 px); in-place content swap `rise_in(id, TRANSCRIPT_IN, TRANSCRIPT_IN_RISE, el)` |
| Status / label swap | `status_swap(loaders::label_id("run-status", &label), el)`: `STATUS_SWAP` 180 ms quint + 2 px settle |
| Resort a list | `RESORT` 260 ms quint |
| Pane resize / collapse / sheets | `RESIZE` 200 ease-out / `COLLAPSE` 180 / `EMPHASIZED` 360 |
| Opacity only (safe on absolute elements) | `fade_in` = `fade_quick` (150 ms ease) |
| Any exit | `exit_of(spec)`: 0.75× duration, accelerate curve |

The rising helpers set `.relative()`. On absolutely positioned elements, call `with_animation`
with `spec.animation()` directly. Pink never overshoots, so `EASE_OUT_BACK` is off limits.
`GLIDE`, `ROW_IN`, `VIEW_IN` and `FILL` are legacy aliases. Use the names above.

### Pulse clock and loaders

Every loop runs on Pink's shared pulse clock: ~30 fps (15 Hz for coarse loaders), phase-locked
across instances, parked when nothing is mounted. Never loop with `with_animation(..).repeat()` or
a timer task. The kit's loaders need no window or cx. For a custom loop, read
`motion::pulse_phase(&SPEC, window, cx)` (a phase in `[0, 1)`) while rendering, which also keeps
the view ticking. `pulse_phase_slow` is the 15 Hz form, and `pulse_keep` ticks without a phase.

| State | Loader |
|---|---|
| Something is working (the thinking line), a live/presence dot | `breathing_orb(t.accent)`: 12 px box, core 6 px, halo accent .16 → .34, 1600 ms |
| An item is running | `mini_glyph_spinner(key, 2.0, t.glyph)` (750 ms ring chase; unique `key` per sibling) |
| Running on a solid plate, or neutral | `mini_mono_spinner(key, 2.0, t.on_solid \| t.text_muted)` |
| A pane is loading | `gradient_spinner(3.0, t.glyph)` (2.5 for splash-sized) |
| A list or menu is loading | `skeleton_rows` / `skeleton_menu_rows` / `skeleton_bar` (2400 ms, 0.08 stagger) |
| A live label (tool-group title) | `shimmer_text(label, 12., 18., t.content_muted, t.text)`, then `.flex_none()` or `.flex_1().min_w_0().overflow_hidden()` |
| Upload progress | `upload_progress_ring(percent, 34.)` over a dimmed image |
| Boot | `splash_overlay(bg, t.glyph, label, label_color, fading)` |

**The working trailer** is Pink's thinking indicator. It is one bare line under the last item:
no card, no spinner ring, no progress bar.

```rust
div().id("working-trailer").group("working-trailer")
    .flex().flex_row().items_start().gap(px(8.)).pt(px(16.)).min_w_0()
    .child(loaders::orb_slot(t.accent))
    .child(loaders::working_status("Reading", t.content_muted, t.text).object("motion.rs"))
    .child(loaders::working_timer(secs, t.content_faint))          // hide while "Sending"
    .child(div().id("working-stop").tab_index(0).ml(px(4.)).px(px(6.)).rounded(px(6.))
        .text_size(px(11.)).line_height(px(16.)).text_color(t.content_muted)
        .opacity(0.).group_hover("working-trailer", |s| s.opacity(1.))
        .focus_visible(|s| s.opacity(1.))
        .hover_wash("working-stop", t.fill(FillTier::Tertiary)).child("Stop"))
```

The verb shimmers (3400 ms) from `content_muted` toward `text`. The object is `text` and truncates
at 44 characters. Each new label replays `STATUS_SWAP`. Queued and provider notices use
`.sentence(true)` with tone `t.warning`: they wrap and don't shimmer.

### Reduced motion

`motion::set_reduced_motion(cx, true)` sets GPUI's own flag. Every `with_animation` then snaps
(oneshots to their end, loops to their start), the pulse clock parks, hover fades and followers
snap, the orb rests full, and shimmer becomes plain text. For any loop of your own, check
`motion::reduced_motion(cx)` and skip it. `PINK_UI_MOTION_SCALE` slows everything down for review.

## 6. Icons

| Topic | Convention |
|---|---|
| Style | Solar Icons, Linear: `viewBox="0 0 24 24"`, `fill="none"`, `stroke="currentColor"`, `stroke-width="1.5"`, round caps and joins, `width/height="1em"`. Hand-drawn glyphs follow the same rules. Filled glyphs only for states (stop square, starred). |
| Pink's files | The app's `crates/ui/assets/icons/*.svg` (licenses in `icons/licenses/`); file-type icons in `crates/ui/assets/file-icons` are Symbols (MIT). Solar by 480 Design (CC BY 4.0), except `webhook`, `package-search`, `git-pull-request` (Lucide, ISC), `arrow-fork` (Tabler, MIT), `mcp-mark` (Simple Icons, CC0). Don't copy brand marks (`pink-logo`, `*-mark`). |
| In pink-ui | Copy what you use into `assets/icons/<slug>/` and reference `"icons/<slug>/copy.svg"`. Solar, Symbols, Lucide, Tabler and Simple Icons are already credited in `THIRD_PARTY_NOTICES.md` (license texts in `assets/icons/licenses/`); credit any other source you add. |
| Common names | `alt-arrow-down` (pop-up), `alt-arrow-right` (submenu, disclosure), `check`, `close`, `copy`, `danger-circle` (failed), `danger-triangle` (notice), `info-circle`, `restart` (retry), `magnifer` / `palette-search`, `shield-warning` (approval), `chat-round-question-mark` (input), `more-horizontal`, `pin`, `trash-bin-minimalistic`, `stop` |
| Rendering | `svg().path(p).size(px(icon_size::SM)).flex_none().text_color(tone)`. Always set the color, because SVGs don't inherit it. In controls, `pink::glyph(p, size)` sizes the icon and applies `glyph_tone()`. For check, chevrons, close and plus, `pink::line_glyph(pink::glyphs::CHECK, 14., color)` needs no asset. |
| Sizes | 12 beside caption text, 14 in toolbars, chips and chevrons, 16 in buttons, menus and tiles, 20 in the titlebar, 28 in an empty state |

## 7. Review checklist: what Pink avoids

| Check | Pink avoids | Do instead |
|---|---|---|
| [ ] | Decorative gradients | `linear_gradient` only for functional fades (the opaque edge fade that stands in for `EdgeFade`) |
| [ ] | Shadows on in-flow content | floating cards only: `menu_shadows`, `tooltip_shadows`, `shadow_lg`; never `primitives::shadow` (halves blur) or `card_shadow()` (empty) |
| [ ] | Opaque or saturated state fills | translucent washes; status fills ≤ 0.12–0.17; solid plates only for Primary (`text`), Danger and a send disc |
| [ ] | Accent as decoration | accent only for focus and selection rings, caret, working orb, unread dots, switch tracks, selected chip wash, viewed/focused tab; no accent buttons, headings, icons, meters or progress hairlines |
| [ ] | Bold | 700 only in `Display`; titles and section labels 600; rows, buttons, chips 500 |
| [ ] | Uppercase | only `menu_heading` (and a question-wizard header), tracked with `tracked_upper` |
| [ ] | Borders that shift layout | 2 px inset focus ring; transparent 1 px border at rest; inset selection ring |
| [ ] | Ellipses on sidebar-style list titles | fade the last 20 px (pink-ui: a gradient overlay to the row color) |
| [ ] | Overshoot, bounce, spring pops | `EASE_OUT_BACK` is banned; use the decelerating presets |
| [ ] | Colored rows for failures | one red glyph and one red word; parent headers stay quiet |
| [ ] | Hover on non-controls | settings rows and static cards stay still |
| [ ] | Hover that looks like selection | hover ≈ 60% of the selected wash, or one fill tier lower |
| [ ] | Celebratory success (green check tiles, pops) | done is quiet: a dot, or nothing |
| [ ] | Raw secondary text, hex literals, pure white/black washes | `readable_*`, tokens, `fill` / `ink` / `wash` / `hairline` |
| [ ] | Private loops, GPUI `.hover()` | the pulse clock, `HoverFill`, reduced motion respected |
| [ ] | Loose fonts and sizes | `pink::SANS` at the root, ladder sizes only, no 16 px text |

## 8. Example: a runs list

A minimal Pink component: clickable rows with live status, a ticking timer and a hover-revealed
action, built as a `RenderOnce` builder the way `CONTRIBUTING.md` describes.

```rust
use std::rc::Rc;
use gpui::{AnyElement, App, ElementId, FontWeight, SharedString, Window, div, prelude::*, px, svg};
use crate::{HoverFill, pink::{self, ButtonKind, ControlSize, FillTier, Pink, TextStyle, TypeStyleExt, loaders}};

#[derive(Clone, Copy, PartialEq)]
pub enum RunState { Running, Waiting, Failed, Done }
pub struct Run { pub title: SharedString, pub state: RunState, pub secs: i64 }

#[derive(IntoElement)]
pub struct Runs { id: ElementId, runs: Vec<Run>, selected: Option<usize>,
    on_select: Option<Rc<dyn Fn(usize, &mut Window, &mut App)>> }

impl RenderOnce for Runs {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let ring = t.focus_ring();
        let Runs { id, runs, selected, on_select } = self;
        pink::card(&t).id(id).font_family(pink::SANS).w(px(360.))
            .p(px(pink::layout::CARD_INSET)).flex().flex_col().gap(px(pink::layout::MENU_GAP))
            .child(div().flex().items_center().px(px(8.)).pt(px(6.)).pb(px(4.))
                .child(div().flex_1().type_style(TextStyle::Caption).font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.readable_faint(0.9)).child("Runs"))
                .child(pink::button("runs-new", ButtonKind::Ghost, ControlSize::Sm, &t).child("New run")))
            .children(runs.into_iter().enumerate().map(|(ix, run)| {
                let key = format!("runs-row-{ix}");
                let wash = t.fill(if selected == Some(ix) { FillTier::Primary } else { FillTier::Tertiary });
                let rest = if selected == Some(ix) { wash } else { wash.opacity(0.) };
                div().id(("runs-row", ix)).group(key.clone()).tab_index(0)
                    .h(px(32.)).px(px(8.)).flex().items_center().gap(px(8.))
                    .rounded(px(pink::radius::MENU_ITEM)).text_size(px(pink::text::BODY))
                    .hover_fill_fg(key.clone(), rest, wash, t.text.opacity(0.9), t.text)
                    .focus_visible(move |s| pink::focus_ring(s, ring))
                    .child(status_mark(run.state, ix, &t))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(run.title))
                    .child(if run.state == RunState::Failed {
                        pink::status_pill("Failed", t.danger, None, &t).into_any_element()
                    } else {
                        div().flex_none().text_size(px(pink::text::CAPTION))
                            .font_features(pink::tabular_figures()).text_color(t.content_faint)
                            .child(loaders::format_elapsed(run.secs)).into_any_element()
                    })
                    .child(pink::icon_button(format!("runs-more-{ix}"), "icons/runs/more-horizontal.svg",
                            ControlSize::Sm, &t)
                        .opacity(0.).group_hover(key, |s| s.opacity(1.))
                        .focus_visible(move |s| pink::focus_ring(s.opacity(1.), ring))
                        .tooltip(pink::text_tooltip("More"))
                        .tooltip_show_delay(pink::layout::TOOLTIP_DELAY))
                    .when_some(on_select.clone(), |row, f| row.on_click(move |_, w, cx| f(ix, w, cx)))
            }))
    }
}

/// Shape before color: spinner, waiting glyph, red glyph, quiet dot, in one 16 px slot.
fn status_mark(state: RunState, ix: usize, t: &Pink) -> AnyElement {
    let slot = div().size(px(16.)).flex_none().flex().items_center().justify_center();
    let icon = |path: &'static str, tone| svg().path(path).size(px(12.)).text_color(t.readable_icon(tone));
    match state {
        RunState::Running => slot.child(loaders::mini_glyph_spinner(format!("runs-spin-{ix}"), 2.0, t.glyph)),
        RunState::Waiting => slot.child(icon("icons/runs/shield-warning.svg", t.warning)),
        RunState::Failed => slot.child(icon("icons/runs/danger-circle.svg", t.danger)),
        RunState::Done => slot.child(div().size(px(6.)).rounded_full().bg(t.success.opacity(0.9))),
    }
    .into_any_element()
}
```

To finish it, add `NAME`, `DESCRIPTION` and a `story` with realistic data, copy the three Solar
icons into `assets/icons/runs/`, animate new rows on `TOOL_ROW_REVEAL`, and check it in both
modes with reduced motion on.
