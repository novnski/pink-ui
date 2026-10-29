//! Pink's control kit: buttons, icon buttons, pop-up buttons, chips,
//! segmented controls, switches, checkboxes, badges, pills, key hints and
//! identity tiles (Pink `button.rs`, `settings/widgets.rs`, `popover.rs`,
//! `pickers.rs`, `notice.rs`, `shell.rs`).
//!
//! Every control in the kit shares one frame per [`ControlSize`]: one height,
//! corner, type size and line height; a transparent 1 px border at rest so
//! outlined and plain kinds measure the same; a tab stop with the 2 px inset
//! focus ring; Enter/Space reaching `on_click` exactly once. Hovers ease
//! through [`HoverFill`] (Pink's 150 ms `transition-colors`).
//!
//! The `*_enabled` variants take Pink's `enabled` flag: a disabled control
//! paints its resting colors at 45% opacity, with no hover, pointer or tab
//! stop.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Canvas, Div, FontWeight, Hsla, KeyDownEvent, Role, SharedString,
    Stateful, Svg, Toggled, Window, canvas, div, point, prelude::*, px, svg,
};

use super::{FillTier, MONO, Pink, focus_ring, radius};
use crate::{
    HoverFill,
    components::menu_card as menu,
    motion::{self, DISCLOSURE, QUICK},
};

/// Stops Enter/Space key-downs on a push button so GPUI's single keyboard
/// click is the only activation (Pink `swallow_activation_key`).
pub fn swallow_activation_key(event: &KeyDownEvent, _: &mut Window, cx: &mut App) {
    if matches!(event.keystroke.key.as_str(), "enter" | "space") {
        cx.stop_propagation();
    }
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

// ---------------------------------------------------------------------------
// Glyphs
// ---------------------------------------------------------------------------

/// Stroke outlines of the Solar Linear glyphs the kit draws itself (24×24
/// viewBox, 1.5 stroke, round caps and joins), for [`line_glyph`]. Each entry
/// is one polyline.
pub mod glyphs {
    /// `check.svg`: `M5 13L9 17L19 7`.
    pub const CHECK: &[&[(f32, f32)]] = &[&[(5.0, 13.0), (9.0, 17.0), (19.0, 7.0)]];
    /// `alt-arrow-down.svg`: `m19 9l-7 6l-7-6` (pop-up chevron).
    pub const ALT_ARROW_DOWN: &[&[(f32, f32)]] = &[&[(19.0, 9.0), (12.0, 15.0), (5.0, 9.0)]];
    /// `alt-arrow-up.svg`.
    pub const ALT_ARROW_UP: &[&[(f32, f32)]] = &[&[(19.0, 15.0), (12.0, 9.0), (5.0, 15.0)]];
    /// `alt-arrow-right.svg`: `m9 5l6 7l-6 7` (submenu, disclosure).
    pub const ALT_ARROW_RIGHT: &[&[(f32, f32)]] = &[&[(9.0, 5.0), (15.0, 12.0), (9.0, 19.0)]];
    /// `alt-arrow-left.svg`.
    pub const ALT_ARROW_LEFT: &[&[(f32, f32)]] = &[&[(15.0, 5.0), (9.0, 12.0), (15.0, 19.0)]];
    /// `close.svg`'s cross.
    pub const CLOSE: &[&[(f32, f32)]] = &[&[(6.0, 6.0), (18.0, 18.0)], &[(18.0, 6.0), (6.0, 18.0)]];
    /// A plus.
    pub const PLUS: &[&[(f32, f32)]] =
        &[&[(12.0, 5.0), (12.0, 19.0)], &[(5.0, 12.0), (19.0, 12.0)]];
}

/// A Solar-style line glyph painted directly (no asset): `strokes` in a
/// 24×24 box scaled to `size`, with the 1.5 stroke scaled alike and round
/// caps and joins. Use it for the kit's checks and chevrons.
pub fn line_glyph(strokes: &'static [&'static [(f32, f32)]], size: f32, color: Hsla) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds: Bounds<gpui::Pixels>, _, window, _| {
            let scale = f32::from(bounds.size.width) / 24.0;
            let width = 1.5 * scale;
            let at = |(x, y): (f32, f32)| {
                point(
                    bounds.origin.x + px(x * scale),
                    bounds.origin.y + px(y * scale),
                )
            };
            for stroke in strokes {
                let mut path = gpui::PathBuilder::stroke(px(width));
                for (ix, &p) in stroke.iter().enumerate() {
                    if ix == 0 {
                        path.move_to(at(p));
                    } else {
                        path.line_to(at(p));
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
                // Round caps and joins: a dot of the stroke width at each vertex.
                for &p in stroke.iter() {
                    let dot = Bounds::centered_at(at(p), gpui::size(px(width), px(width)));
                    window.paint_quad(gpui::fill(dot, color).corner_radii(px(width / 2.0)));
                }
            }
        },
    )
    .size(px(size))
    .flex_none()
}

/// A leading glyph sized for `size`, in the default glyph tone
/// (`readable_muted(0.95)` of the last [`Pink::get`]). GPUI paints an SVG only
/// with its own text color, so inside a Primary or Danger button chain
/// `.text_color(t.on_solid)` / `.text_color(t.on_danger())`.
pub fn glyph(path: impl Into<SharedString>, size: ControlSize) -> Svg {
    svg()
        .path(path)
        .size(px(size.glyph()))
        .flex_none()
        .text_color(Pink::current().glyph_tone())
}

// ---------------------------------------------------------------------------
// Sizes and the shared frame
// ---------------------------------------------------------------------------

/// The control size ladder (Pink `button::ControlSize`), from the
/// `pink-button` package (`components/button`).
///
/// | Size | Height | Radius | Type  | Side pad | Glyph | Gap |
/// | ---- | ------ | ------ | ----- | -------- | ----- | --- |
/// | `Sm` | 24     | 6      | 11/14 | 8        | 12    | 4   |
/// | `Md` | 28     | 8      | 12/16 | 10       | 14    | 6   |
/// | `Lg` | 32     | 8      | 13/18 | 12       | 16    | 6   |
///
/// `Sm` is for toolbars, inline row actions and chips, `Md` for the settings
/// control column and panel headers, `Lg` for dialogs and the sidebar footer.
pub use crate::components::button::ControlSize;
use crate::components::button::{self as control, ControlStyle};

/// The semantics every control adds over the package's frame (Pink
/// `control_frame_inner`): the button role, the tab stop and the 2 px inset
/// focus ring. `swallow_keys`: push buttons stop Enter/Space key-downs;
/// pop-up triggers own their keys and opt out.
fn control_semantics(
    frame: Stateful<Div>,
    enabled: bool,
    swallow_keys: bool,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    frame
        .role(Role::Button)
        .when(enabled, |el| {
            el.tab_index(0)
                .focus_visible(move |style| focus_ring(style, ring))
        })
        .when(enabled && swallow_keys, |el| {
            el.on_key_down(swallow_activation_key)
        })
}

/// A control's colours under its hover fade: the plate and label mixed from
/// rest to hover by `key`'s fade while enabled, the resting ones otherwise.
fn hover_style(
    key: &str,
    size: ControlSize,
    enabled: bool,
    colors: (Hsla, Hsla, Hsla, Hsla),
    border: Hsla,
    weight: FontWeight,
) -> ControlStyle {
    let (rest, hover, fg, hover_fg) = colors;
    let mix = if enabled { motion::hover_t(key) } else { 0.0 };
    ControlStyle {
        size,
        fill: Some(motion::mix(rest, hover, mix)),
        text: motion::mix(fg, hover_fg, mix),
        border,
        weight,
        enabled,
    }
}

/// Adds the hover listener behind [`hover_style`]'s fade to an enabled control.
fn with_hover(frame: Stateful<Div>, key: String, enabled: bool) -> Stateful<Div> {
    frame.when(enabled, |el| el.on_hover(motion::hover_listener(key)))
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

/// Button emphasis (Pink `button::ButtonKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The one solid action in a group: a `text`-colored plate.
    Primary,
    /// A filled, outlined push button.
    Secondary,
    /// A quiet action on a faint resting plate, so it never reads as body text.
    Ghost,
    /// Kept for older call sites. Pink has no accent-filled button (accent
    /// appears only as washes and rings), so this paints as [`Primary`].
    ///
    /// [`Primary`]: ButtonKind::Primary
    Accent,
    /// A destructive action.
    Danger,
}

struct KindColors {
    rest: Hsla,
    hover: Hsla,
    fg: Hsla,
    hover_fg: Hsla,
    border: Option<Hsla>,
}

fn kind_colors(kind: ButtonKind, t: &Pink) -> KindColors {
    match kind {
        ButtonKind::Primary | ButtonKind::Accent => KindColors {
            rest: t.text,
            hover: t.text.opacity(0.86),
            fg: t.on_solid,
            hover_fg: t.on_solid,
            border: None,
        },
        ButtonKind::Secondary => KindColors {
            rest: t.fill(FillTier::Quaternary),
            hover: t.fill(FillTier::Tertiary),
            fg: t.text,
            hover_fg: t.text,
            border: Some(t.separator(true)),
        },
        ButtonKind::Ghost => KindColors {
            rest: t.fill(FillTier::Quaternary),
            hover: t.fill(FillTier::Tertiary),
            fg: t.readable_muted(0.95),
            hover_fg: t.text,
            border: None,
        },
        ButtonKind::Danger => KindColors {
            rest: t.danger_strong,
            hover: t.danger_strong.opacity(0.86),
            fg: t.on_danger(),
            hover_fg: t.on_danger(),
            border: None,
        },
    }
}

/// A push button. Add the label (and an optional leading [`glyph`]) and the
/// click handler: `button("save", ButtonKind::Primary, ControlSize::Md, &t).child("Save")`.
pub fn button(
    id: impl Into<SharedString>,
    kind: ButtonKind,
    size: ControlSize,
    t: &Pink,
) -> Stateful<Div> {
    button_enabled(id, kind, size, true, t)
}

/// [`button`] with Pink's `enabled` flag.
pub fn button_enabled(
    id: impl Into<SharedString>,
    kind: ButtonKind,
    size: ControlSize,
    enabled: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let colors = kind_colors(kind, t);
    let key = format!("pink-btn-{id}");
    let style = hover_style(
        &key,
        size,
        enabled,
        (colors.rest, colors.hover, colors.fg, colors.hover_fg),
        colors.border.unwrap_or(gpui::transparent_black()),
        FontWeight::MEDIUM,
    );
    let frame = control_semantics(control::button(id, &style), enabled, true, t);
    with_hover(frame, key, enabled)
}

/// A square glyph-only button: transparent at rest, `fill(Tertiary)` under
/// the pointer, the glyph brightening from `readable_muted(0.95)` to `text`.
/// Add the accessible name and tooltip.
pub fn icon_button(
    id: impl Into<SharedString>,
    glyph_path: impl Into<SharedString>,
    size: ControlSize,
    t: &Pink,
) -> Stateful<Div> {
    icon_button_enabled(id, glyph_path, size, true, t)
}

/// [`icon_button`] with Pink's `enabled` flag.
pub fn icon_button_enabled(
    id: impl Into<SharedString>,
    glyph_path: impl Into<SharedString>,
    size: ControlSize,
    enabled: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let key = format!("pink-icon-{id}");
    let rest_fg = t.readable_muted(0.95);
    let tone = if enabled {
        motion::hover_blend(&key, rest_fg, t.text)
    } else {
        rest_fg
    };
    let hover = t.fill(FillTier::Tertiary);
    let mix = if enabled { motion::hover_t(&key) } else { 0.0 };
    let style = ControlStyle {
        size,
        fill: enabled.then(|| motion::mix(hover.opacity(0.0), hover, mix)),
        text: motion::mix(rest_fg, t.text, mix),
        border: gpui::transparent_black(),
        weight: FontWeight::MEDIUM,
        enabled,
    };
    let frame = control_semantics(control::square(id, &style), enabled, true, t)
        .child(glyph(glyph_path, size).text_color(tone));
    with_hover(frame, key, enabled)
}

/// A pop-up button: a Secondary control showing the current choice and one
/// chevron (Pink `button::select`). Add the click that opens the menu.
pub fn select(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    size: ControlSize,
    open: bool,
    t: &Pink,
) -> Stateful<Div> {
    select_frame(id, size, open, true, t)
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
        .child(select_chevron(t))
}

/// The pop-up button's single chevron: `alt-arrow-down` at 14 in
/// `readable_muted(0.85)`.
pub fn select_chevron(t: &Pink) -> Canvas<()> {
    line_glyph(glyphs::ALT_ARROW_DOWN, 14.0, t.readable_muted(0.85))
}

/// The pop-up button's frame without content, for triggers that show more
/// than a label; add [`select_chevron`] last. `pl pad_x`, `pr pad_x − 2`,
/// gap 8, regular weight; while `open` it rests on the hover fill. The
/// trigger keeps its own key handling.
pub fn select_frame(
    id: impl Into<SharedString>,
    size: ControlSize,
    open: bool,
    enabled: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let colors = kind_colors(ButtonKind::Secondary, t);
    let key = format!("pink-select-{id}");
    let style = hover_style(
        &key,
        size,
        enabled,
        (
            if open { colors.hover } else { colors.rest },
            colors.hover,
            colors.fg,
            colors.fg,
        ),
        colors.border.unwrap_or(gpui::transparent_black()),
        FontWeight::NORMAL,
    );
    let frame =
        control_semantics(control::popup(id, &style), enabled, false, t).aria_expanded(open);
    with_hover(frame, key, enabled)
}

/// A selectable chip for a choice among many: a quiet filled control
/// (`fill(Quaternary)` → `Tertiary`, `separator(false)` edge,
/// `readable_muted(0.95)` → `text`) that takes the accent wash, an
/// `accent @ 0.55` edge and a medium label when chosen. Two to five fixed
/// options use [`segmented`] instead.
pub fn chip(
    id: impl Into<SharedString>,
    selected: bool,
    size: ControlSize,
    t: &Pink,
) -> Stateful<Div> {
    chip_enabled(id, selected, size, true, t)
}

/// [`chip`] with Pink's `enabled` flag.
pub fn chip_enabled(
    id: impl Into<SharedString>,
    selected: bool,
    size: ControlSize,
    enabled: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let (rest, hover, fg, hover_fg, border) = if selected {
        let label = t.readable_tone(t.accent, 1.0);
        (
            t.accent_wash,
            t.accent_wash,
            label,
            label,
            t.accent.opacity(0.55),
        )
    } else {
        (
            t.fill(FillTier::Quaternary),
            t.fill(FillTier::Tertiary),
            t.readable_muted(0.95),
            t.text,
            t.separator(false),
        )
    };
    let key = format!("pink-chip-{id}");
    let style = hover_style(
        &key,
        size,
        enabled,
        (rest, hover, fg, hover_fg),
        border,
        if selected {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        },
    );
    let frame = control_semantics(control::button(id, &style), enabled, true, t)
        .aria_toggled(toggled(selected));
    with_hover(frame, key, enabled)
}

/// The selection ring for swatches and option cards: a 2 px accent ring 2 px
/// outside `child`, following its corner (`inner_radius + 4`). Unselected,
/// the space stays reserved so choosing never shifts the layout.
pub fn selection_ring(selected: bool, inner_radius: f32, t: &Pink, child: impl IntoElement) -> Div {
    div()
        .flex_none()
        .p(px(2.0))
        .border_2()
        .border_color(if selected {
            t.accent
        } else {
            gpui::transparent_black()
        })
        .rounded(px(inner_radius + 4.0))
        .child(child)
}

/// A composer trigger chip, Pink's "ghost pill" (model, branch, access),
/// drawn by the `pink-trigger-chip` package at the kit's 16 pt baseline
/// (`Units::Pixels`): 28 tall, fully rounded, `px 10`, gap 6, 12 medium; no
/// border, no caret. The label rests at `text @ 0.9` when the choice is
/// `set` (else `text_muted`) and brightens to `text`; the wash is
/// `element_hover` under the pointer and while `open`. Icons inside are 16.
/// The caller chains the icon and label; [`trigger_chip_with`] hands the
/// package its icon, label, badge and suffix, as Pink's chips do.
pub fn trigger_chip(id: impl Into<SharedString>, set: bool, open: bool, t: &Pink) -> Stateful<Div> {
    trigger_chip_with(id, Default::default(), set, open, t)
}

/// [`trigger_chip`] drawing `content` (Pink `Pickers::trigger_chip`): the
/// leading icon, the label, a badge after it, the effort as a muted
/// `readable_muted(0.7)` suffix, or the square icon-only form.
pub fn trigger_chip_with(
    id: impl Into<SharedString>,
    content: crate::components::trigger_chip::TriggerChipContent,
    set: bool,
    open: bool,
    t: &Pink,
) -> Stateful<Div> {
    use crate::components::trigger_chip as chip;
    let id: SharedString = id.into();
    let key = format!("pink-trigger-{id}");
    let ring = t.focus_ring();
    let rest_fg = if set {
        t.text.opacity(0.9)
    } else {
        t.text_muted
    };
    let hover = motion::hover_t(&key);
    let rest_wash = if open {
        t.element_hover
    } else {
        t.element_hover.opacity(0.0)
    };
    chip::trigger_chip(
        id,
        content,
        &chip::TriggerChipStyle {
            units: chip::Units::Pixels,
            label: motion::mix(rest_fg, t.text, hover),
            suffix: t.readable_muted(0.7),
            wash: motion::mix(rest_wash, t.element_hover, hover),
        },
    )
    .role(Role::Button)
    .tab_index(0)
    .focus_visible(move |style| focus_ring(style, ring))
    .on_key_down(swallow_activation_key)
    .aria_expanded(open)
    .on_hover(motion::hover_listener(key))
}

/// A notice's capsule action (Pink `notice::notice_action`), drawn by the
/// `pink-notice` package: 24 tall, `px 10` in rems, fully rounded,
/// `border_strong` edge, medium `text`, the quiet hover wash.
pub fn notice_action(id: impl Into<SharedString>, t: &Pink) -> Stateful<Div> {
    let id: SharedString = id.into();
    let ring = t.focus_ring();
    crate::components::notice::action(&crate::components::notice::ActionStyle {
        border: t.border_strong,
        text: t.text,
    })
    .id(id.clone())
    .role(Role::Button)
    .tab_index(0)
    .focus_visible(move |style| focus_ring(style, ring))
    .on_key_down(swallow_activation_key)
    .hover_wash(format!("pink-notice-{id}"), t.fill(FillTier::Tertiary))
}

// ---------------------------------------------------------------------------
// Segmented control
// ---------------------------------------------------------------------------

/// One option of a [`segmented`] control.
#[derive(Clone, Debug)]
pub struct Segment {
    pub key: SharedString,
    pub label: SharedString,
    pub glyph: Option<SharedString>,
}

impl Segment {
    pub fn new(key: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            glyph: None,
        }
    }

    /// A leading 12 px glyph (an SVG asset path).
    pub fn glyph(mut self, path: impl Into<SharedString>) -> Self {
        self.glyph = Some(path.into());
        self
    }
}

/// A segmented control (Pink `button::segmented`), drawn by the
/// `pink-segmented-control` package: equal-width options on a
/// `fill(Quaternary)` track with a `separator(false)` edge, and one thumb
/// that slides to the chosen option over [`motion::QUICK`] (160 ms). The thumb
/// is `fill(Primary)` on dark; white with a hairline edge and a
/// `0 1 2 black 8%` shadow on light. The chosen label is medium `text`, the
/// others `readable_muted(0.9)` brightening on hover. `width` is the whole
/// control's width in design pt; like the height and the labels it is in
/// rems, so the control grows with the Interface size. `on_select` receives
/// the chosen index.
///
/// Every option is a tab stop chosen with Space, Enter or a click; the arrow
/// keys move the choice and the focus together, wrapping at the ends, and
/// Home and End jump to the first and last option (Pink
/// `segment_key_target`).
pub fn segmented(
    id: impl Into<SharedString>,
    segments: Vec<Segment>,
    selected: usize,
    width: f32,
    size: ControlSize,
    t: &Pink,
    on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    use crate::components::segmented_control::tray;

    let id: SharedString = id.into();
    let count = segments.len().max(1);
    let selected = selected.min(count - 1);
    let position = motion::follow(&format!("pink-seg-{id}-thumb"), selected as f32, QUICK);
    let on_select = Rc::new(on_select);
    let ring = t.focus_ring();
    let muted = t.readable_muted(0.9);
    let options: Vec<AnyElement> = segments
        .into_iter()
        .enumerate()
        .map(|(ix, segment)| {
            let chosen = ix == selected;
            let on_key = on_select.clone();
            let on_select = on_select.clone();
            let key = format!("pink-seg-{id}-{}", segment.key);
            let tone = if chosen { t.text } else { muted };
            tray::option(SharedString::from(key.clone()), chosen, size.radius())
                .role(Role::RadioButton)
                .aria_label(segment.label.clone())
                .aria_toggled(toggled(chosen))
                .tab_index(0)
                .focus_visible(move |style| focus_ring(style, ring))
                .on_key_down(swallow_activation_key)
                .on_key_down(move |event, window, cx| {
                    let Some(target) = segment_key_target(event, ix, count) else {
                        return;
                    };
                    cx.stop_propagation();
                    if target != selected {
                        on_key(&target, window, cx);
                    }
                    // The options are consecutive tab stops, so the focus
                    // walks from this one to the target's.
                    for _ in ix.min(target)..ix.max(target) {
                        if target > ix {
                            window.focus_next(cx);
                        } else {
                            window.focus_prev(cx);
                        }
                    }
                })
                .hover_fg(key, tone, t.text)
                .on_click(move |_, window, cx| on_select(&ix, window, cx))
                .when_some(segment.glyph, |el, path| {
                    el.child(glyph(path, ControlSize::Sm).text_color(tone))
                })
                .child(tray::label(segment.label))
                .into_any_element()
        })
        .collect();
    let thumb = tray::thumb(
        position,
        count,
        size.radius(),
        if t.dark {
            t.fill(FillTier::Primary)
        } else {
            gpui::white()
        },
        (!t.dark).then(|| tray::Lift {
            edge: t.separator(false),
            shadows: vec![gpui::BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.08),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(2.0),
                spread_radius: px(0.0),
                inset: false,
            }],
        }),
    );
    tray::tray(
        &tray::Track {
            width: Some(width),
            height: size.height(),
            radius: size.radius(),
            text: size.text(),
            fill: t.fill(FillTier::Quaternary),
            edge: t.separator(false),
        },
        Some(thumb),
        options,
    )
    .text_color(t.text)
    .id(id)
    .role(Role::RadioGroup)
}

/// The option an unmodified arrow, Home or End key picks from the focused
/// option `from` among `count` options, wrapping at the ends (Pink
/// `segment_key_target`); `None` for any other key.
fn segment_key_target(event: &KeyDownEvent, from: usize, count: usize) -> Option<usize> {
    if event.keystroke.modifiers.modified() {
        return None;
    }
    match event.keystroke.key.as_str() {
        "left" | "up" => Some((from + count - 1) % count),
        "right" | "down" => Some((from + 1) % count),
        "home" => Some(0),
        "end" => Some(count - 1),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Switch and checkbox
// ---------------------------------------------------------------------------

/// The switch itself, display only (Pink `settings::widgets::toggle_switch`):
/// a 34×20 pill, off `fill(Primary)` (light `text_muted @ 0.45`), on
/// `accent_strong`, with a white 16 px knob and `shadow_sm`. Knob and track
/// glide over [`motion::DISCLOSURE`] (300 ms) only when the value flips. `key`
/// names the switch's motion; keep it unique on screen.
pub fn toggle_switch(on: bool, key: &str, t: &Pink) -> Div {
    let track_off = if t.dark {
        t.fill(FillTier::Primary)
    } else {
        t.text_muted.opacity(0.45)
    };
    let p = motion::follow(
        &format!("pink-switch-{key}"),
        if on { 1.0 } else { 0.0 },
        DISCLOSURE,
    );
    crate::components::switch::switch(
        &crate::components::switch::SwitchStyle {
            track: motion::mix(track_off, t.accent_strong, p),
            knob: gpui::white(),
        },
        p,
    )
}

/// [`toggle_switch`] as a control (Pink `switch_control`): an announced,
/// focusable Switch named `label`. Add `.on_click(..)` to flip it. The focus
/// ring is a transparent 2 px border that turns the ring color, so focusing
/// never moves the row.
pub fn switch_control(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let ring = t.focus_ring();
    div()
        .id(id.clone())
        .role(Role::Switch)
        .aria_label(label)
        .aria_toggled(toggled(on))
        .tab_index(0)
        .flex_none()
        .flex()
        .rounded_full()
        .border_2()
        .border_color(gpui::transparent_black())
        .focus_visible(move |style| style.border_color(ring))
        .cursor_pointer()
        .on_key_down(swallow_activation_key)
        .child(toggle_switch(on, &id, t))
}

/// [`switch_control`] while it can't change: the same geometry and motion
/// key, announced with `reason`, with no tab stop or pointer.
pub fn switch_control_disabled(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on: bool,
    reason: impl Into<SharedString>,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    div()
        .id(id.clone())
        .role(Role::Switch)
        .aria_label(label)
        .aria_toggled(toggled(on))
        .aria_description(reason)
        .flex_none()
        .flex()
        .rounded_full()
        .border_2()
        .border_color(gpui::transparent_black())
        .cursor_default()
        .child(toggle_switch(on, &id, t))
}

/// The checkbox square (Pink's dialog checkbox): 16 px, radius 4, 1 px edge;
/// unchecked `border` over the hover wash, checked the accent with a 12 px
/// check in `on_solid`.
pub fn checkbox_box(checked: bool, t: &Pink) -> Div {
    div()
        .flex_none()
        .size(px(16.0))
        .rounded(px(radius::XS))
        .border_1()
        .border_color(if checked { t.accent } else { t.border })
        .bg(if checked { t.accent } else { t.glass_hover() })
        .flex()
        .items_center()
        .justify_center()
        .when(checked, |el| {
            el.child(line_glyph(glyphs::CHECK, 12.0, t.on_solid))
        })
}

/// A labelled checkbox row (Pink's "Don't show again"): an announced CheckBox
/// with the box, gap 8, and the label in dialog body type (13/19
/// `text_muted`). The row's 4 px inset is cancelled by its margin so the
/// focus ring clears the box; it is at least `chrome::MIN_HIT` tall, so the
/// target is never under 24×24. Add `.on_click(..)`.
pub fn checkbox(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    checked: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    // Pink rings this one in the raw accent, not the floored focus color.
    let ring = t.accent;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.0))
        .role(Role::CheckBox)
        .aria_label(label.clone())
        .aria_toggled(toggled(checked))
        .tab_index(0)
        .rounded(px(6.0))
        .px(px(4.0))
        .mx(px(-4.0))
        .min_h(px(super::chrome::MIN_HIT))
        .focus_visible(move |style| focus_ring(style, ring))
        .cursor_pointer()
        .on_key_down(swallow_activation_key)
        .child(checkbox_box(checked, t))
        .child(
            div()
                .text_size(px(13.0))
                .line_height(px(19.0))
                .text_color(t.text_muted)
                .child(label),
        )
}

// ---------------------------------------------------------------------------
// Badges and pills
// ---------------------------------------------------------------------------

/// Tone of a [`badge`] or pill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

impl Tone {
    /// The tone's base color.
    pub fn color(self, t: &Pink) -> Hsla {
        match self {
            Tone::Neutral => t.text_muted,
            Tone::Accent => t.accent,
            Tone::Success => t.success,
            Tone::Warning => t.warning,
            Tone::Danger => t.danger,
        }
    }

    /// The tone's copy color (`*_muted`; the accent and neutral have none).
    pub fn muted(self, t: &Pink) -> Hsla {
        match self {
            Tone::Neutral => t.text_muted,
            Tone::Accent => t.accent,
            Tone::Success => t.success_muted,
            Tone::Warning => t.warning_muted,
            Tone::Danger => t.danger_muted,
        }
    }
}

/// A status badge, Pink's settings pill family: fully rounded, `px 8 / py 2`,
/// 11 regular.
///
/// - `Neutral` (`widgets::badge`): a `border` outline, `text_muted` label.
/// - `Success` (`widgets::badge_active`): a `success @ 0.12` fill, the label
///   `readable_tone(success_muted, 0.9)`.
/// - `Danger` / `Warning` (`project_commands`): the outline in `tone @ 0.35`,
///   the label `readable_tone(tone_muted, 0.9)`.
/// - `Accent` has no Pink badge; it follows the outlined family in the accent.
///
/// Drawn by `pink-settings-parts` (`parts::badge`); the kit centres the
/// label in a flex box, so a badge stretched by its row keeps it centred.
pub fn badge(label: impl Into<SharedString>, tone: Tone, t: &Pink) -> Div {
    use crate::components::settings_parts as parts;
    let style = match tone {
        Tone::Neutral => parts::BadgeStyle {
            edge: Some(t.border),
            fill: None,
            text: t.text_muted,
        },
        Tone::Success => parts::BadgeStyle {
            edge: None,
            fill: Some(t.success.opacity(0.12)),
            text: t.readable_tone(t.success_muted, 0.9),
        },
        Tone::Accent | Tone::Warning | Tone::Danger => parts::BadgeStyle {
            edge: Some(tone.color(t).opacity(0.35)),
            fill: None,
            text: t.readable_tone(tone.muted(t), 0.9),
        },
    };
    parts::badge(&style, label).flex().items_center()
}

/// A status pill (Pink's sidebar waiting pill: "Approve", "Reply",
/// "Failed"): 16 tall, `px 5`, gap 3, radius 4, a `tone @ 0.12` fill, 11/14
/// medium, the word and an optional 11 px glyph floored on that fill.
pub fn status_pill(
    label: impl Into<SharedString>,
    tone: Hsla,
    glyph_path: Option<SharedString>,
    t: &Pink,
) -> Div {
    let tone = tone.opacity(1.0);
    let backing = tone.opacity(0.12);
    div()
        .h(px(16.0))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.0))
        .px(px(5.0))
        .rounded(px(radius::XS))
        .bg(backing)
        .text_size(px(11.0))
        .line_height(px(14.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.readable_on(tone, backing, super::TEXT_CONTRAST_TARGET))
        .when_some(glyph_path, |el, path| {
            el.child(
                svg()
                    .path(path)
                    .size(px(11.0))
                    .flex_none()
                    .text_color(t.readable_on(tone, backing, super::NON_TEXT_CONTRAST_TARGET)),
            )
        })
        .child(label.into())
}

/// A message badge (Pink `badges::render`), drawn by the
/// `pink-message-badge` package: at least 24 tall, `px 8`, gap 6 in rems,
/// radius 8, an `ink(0.06)` plate, 12 medium `text_muted`. A leading icon is
/// 12 (`BADGE_GLYPH`) in `readable_muted(0.7)`.
pub fn message_badge(t: &Pink) -> Div {
    crate::components::message_badge::badge(&crate::components::message_badge::BadgeStyle {
        fill: t.ink(0.06),
        text: t.text_muted,
    })
    .flex_none()
}

// ---------------------------------------------------------------------------
// Keyboard hints
// ---------------------------------------------------------------------------

/// A key legend's paint on Pink's theme (Pink popover.rs): the
/// `fill(Quaternary)` cap, the popup's `text_muted` (`for_popup`) for glyphs,
/// words and verbs, a `separator(false)` hairline and Geist Mono.
fn key_style(t: &Pink) -> menu::KeyHintStyle {
    menu::KeyHintStyle {
        cap: t.fill(FillTier::Quaternary),
        text: t.for_popup().text_muted,
        divider: t.separator(false),
        mono: MONO.into(),
    }
}

/// A keyboard hint inside a menu row or tooltip (Pink `popover::kbd_hint`),
/// drawn by the `pink-menu-card` package: `px 5 / py 1`, radius 5,
/// `fill(Quaternary)`, 11 Geist Mono in the popup's `text_muted`
/// (`for_popup`).
pub fn kbd(keys: impl Into<SharedString>, t: &Pink) -> Div {
    menu::kbd_hint(&key_style(t), keys)
}

/// Pink's name for [`kbd`].
pub fn kbd_hint(keys: impl Into<SharedString>, t: &Pink) -> Div {
    kbd(keys, t)
}

/// A footer key cap holding any children (Pink `popover::key_cap`), drawn by
/// the `pink-menu-card` package: 22 tall, `px 5`, radius 5, gap 4,
/// `fill(Quaternary)`. It keeps its width in a squeezed row.
pub fn key_cap(t: &Pink) -> Div {
    menu::key_cap(t.fill(FillTier::Quaternary)).flex_none()
}

/// A footer legend: one glyph key cap (12.5 px, the popup's `text_muted`)
/// and its verb, gap 5 (Pink `popover::key_hint`, drawn by the
/// `pink-menu-card` package).
pub fn key_hint(
    icon_path: impl Into<SharedString>,
    label: impl Into<SharedString>,
    t: &Pink,
) -> Div {
    menu::key_hint(&key_style(t), icon_path, label)
}

/// A footer legend whose cap holds a word ("esc", "tab") in 11 mono.
pub fn key_hint_text(
    cap: impl Into<SharedString>,
    label: impl Into<SharedString>,
    t: &Pink,
) -> Div {
    menu::key_hint_text(&key_style(t), cap, label)
}

/// A footer legend whose cap holds two glyphs split by a hairline
/// ("↑ | ↓ Navigate").
pub fn key_hint_pair(
    first: impl Into<SharedString>,
    second: impl Into<SharedString>,
    label: impl Into<SharedString>,
    t: &Pink,
) -> Div {
    menu::key_hint_pair(&key_style(t), first, second, label)
}

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// A 20 px avatar circle (Pink's history author): `hairline(0.12)` edge,
/// `wash(0.08)` plate, and the initial at 11/18 in `text_faint`, nudged down
/// half a pixel. Put an `img()` child over it for a photo.
pub fn avatar(initial: impl Into<SharedString>, t: &Pink) -> Div {
    div()
        .size(px(20.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded_full()
        .border_1()
        .border_color(t.hairline(0.12))
        .bg(t.wash(0.08))
        .child(
            div()
                .w_full()
                .text_center()
                .text_size(px(11.0))
                .line_height(px(18.0))
                .relative()
                .top(px(0.5))
                .text_color(t.text_faint)
                .child(initial.into()),
        )
}

/// The six project hues Pink assigns in order (violet, green, amber, blue,
/// Pink, coral), as tints for [`monogram_tile`].
pub fn project_tint(index: usize) -> Hsla {
    const HUES: [f32; 6] = [250.0, 150.0, 40.0, 205.0, 325.0, 12.0];
    gpui::hsla(HUES[index % HUES.len()] / 360.0, 0.62, 0.68, 1.0)
}

/// The first letter or digit of `name`, upper-cased.
pub fn monogram(name: &str) -> SharedString {
    name.chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "·".into()) // joiners: allow a lone placeholder glyph for a name with no letter
        .into()
}

/// A project monogram tile (Pink `sidebar_projects::project_tile`): a
/// `size` square, radius `round(size × 0.28)`, `tint @ 0.20` (light 0.22), the
/// monogram semibold at `round(size × 0.5)` in `readable_tone(tint)`.
pub fn monogram_tile(tint: Hsla, name: &str, size: f32, t: &Pink) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px((size * 0.28).round()))
        .bg(tint.opacity(if t.dark { 0.2 } else { 0.22 }))
        .text_size(px((size * 0.5).round()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.readable_tone(tint, 1.0))
        .child(monogram(name))
}
