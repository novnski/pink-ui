// PINK's control kit (button.rs `control_frame`, `button`, `icon_button`,
// `select_frame` and `chip`): the one frame every push button, glyph
// square, pop-up button and chip shares, in three sizes, so every control on
// a pane has one height, corner and type size. UI rethink decision 54: drawn
// here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the plate, label and edge
// colours (already blended over its hover fade), the weight and whether the
// control is enabled, and adds the children, the role, the accessible name,
// Tab focus and its ring, the hover listener, the key handling and the
// actions. Plain `//` comments: the pink-ui gallery compiles this file in
// place with `include!` for its story.
//
// | Size | Height | Radius | Type  | Side pad | Glyph | Gap |
// | ---- | ------ | ------ | ----- | -------- | ----- | --- |
// | `Sm` | 24     | 6      | 11/14 | 8        | 12    | 4   |
// | `Md` | 28     | 8      | 12/16 | 10       | 14    | 6   |
// | `Lg` | 32     | 8      | 13/18 | 12       | 16    | 6   |
//
// The ladder is quoted in pt at the 16 px default. Heights, padding, gaps
// and type are interface rems, so every control grows and shrinks with the
// host's Interface size alongside its label; only corners and hairlines stay
// in pixels.

use gpui::{AbsoluteLength, Div, ElementId, FontWeight, Hsla, Stateful, div, prelude::*, px, rems};

/// The small corner (toolbars, inline row actions, chips).
pub const RADIUS_SM: f32 = 6.0;
/// The medium corner (the settings column, panel headers, dialogs).
pub const RADIUS_MD: f32 = 8.0;
/// A disabled control's strength: its resting colours at 45%.
pub const DISABLED_OPACITY: f32 = 0.45;
/// A pop-up button's gap between its choice and its chevron, in pt.
pub const POPUP_GAP: f32 = 8.0;
/// How much closer a pop-up button's chevron sits to its trailing edge than
/// the label does to the leading one, in pt.
pub const POPUP_TRAIL_TIGHTEN: f32 = 2.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The control size ladder. `Sm` is for toolbars, inline row actions and
/// chips, `Md` for the settings control column and panel headers, `Lg` for
/// dialogs and the sidebar footer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSize {
    Sm,
    Md,
    Lg,
}

impl ControlSize {
    pub const fn height(self) -> f32 {
        match self {
            ControlSize::Sm => 24.0,
            ControlSize::Md => 28.0,
            ControlSize::Lg => 32.0,
        }
    }

    pub const fn radius(self) -> f32 {
        match self {
            ControlSize::Sm => RADIUS_SM,
            ControlSize::Md | ControlSize::Lg => RADIUS_MD,
        }
    }

    pub const fn text(self) -> f32 {
        match self {
            ControlSize::Sm => 11.0,
            ControlSize::Md => 12.0,
            ControlSize::Lg => 13.0,
        }
    }

    pub const fn line_height(self) -> f32 {
        match self {
            ControlSize::Sm => 14.0,
            ControlSize::Md => 16.0,
            ControlSize::Lg => 18.0,
        }
    }

    pub const fn pad_x(self) -> f32 {
        match self {
            ControlSize::Sm => 8.0,
            ControlSize::Md => 10.0,
            ControlSize::Lg => 12.0,
        }
    }

    pub const fn glyph(self) -> f32 {
        match self {
            ControlSize::Sm => 12.0,
            ControlSize::Md => 14.0,
            ControlSize::Lg => 16.0,
        }
    }

    /// The gap between a glyph and the label.
    pub const fn gap(self) -> f32 {
        match self {
            ControlSize::Sm => 4.0,
            ControlSize::Md | ControlSize::Lg => 6.0,
        }
    }
}

/// What one control draws, resolved by the host.
pub struct ControlStyle {
    pub size: ControlSize,
    /// The plate, already blended over the hover fade; `None` paints none
    /// (a disabled glyph square).
    pub fill: Option<Hsla>,
    /// The label colour, already blended over the hover fade. A glyph takes
    /// only its own colour, so the host tints glyphs itself.
    pub text: Hsla,
    /// The 1 px edge. Transparent keeps a plain kind the size of an
    /// outlined one.
    pub border: Hsla,
    pub weight: FontWeight,
    /// An enabled control shows the pointing cursor; a disabled one keeps
    /// its size and rests at [`DISABLED_OPACITY`] with the default cursor.
    pub enabled: bool,
}

/// The frame every control shares: one row of centred content at the size's
/// height, gap, corner, type and line height, inside a 1 px edge. The host
/// chains its width or padding (or uses [`button`], [`square`] or
/// [`popup`]), its children, semantics and actions.
pub fn frame(id: impl Into<ElementId>, style: &ControlStyle) -> Stateful<Div> {
    let size = style.size;
    div()
        .id(id)
        .flex_none()
        .h(ui_rems(size.height()))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(ui_rems(size.gap()))
        .rounded(px(size.radius()))
        .border_1()
        .border_color(style.border)
        .text_size(ui_rems(size.text()))
        .line_height(ui_rems(size.line_height()))
        .font_weight(style.weight)
        .when_some(style.fill, |el, fill| el.bg(fill))
        .text_color(style.text)
        .when(style.enabled, |el| el.cursor_pointer())
        .when(!style.enabled, |el| {
            el.cursor_default().opacity(DISABLED_OPACITY)
        })
}

/// A push button or chip: the frame padded by the size's side pad, as wide
/// as its content.
pub fn button(id: impl Into<ElementId>, style: &ControlStyle) -> Stateful<Div> {
    frame(id, style).px(ui_rems(style.size.pad_x()))
}

/// A glyph-only square: the frame as wide as it is high.
pub fn square(id: impl Into<ElementId>, style: &ControlStyle) -> Stateful<Div> {
    frame(id, style).w(ui_rems(style.size.height()))
}

/// A pop-up button: the frame laid out from its leading edge, the choice
/// then its chevron [`POPUP_GAP`] apart, the chevron's side
/// [`POPUP_TRAIL_TIGHTEN`] tighter than the label's.
pub fn popup(id: impl Into<ElementId>, style: &ControlStyle) -> Stateful<Div> {
    let pad = style.size.pad_x();
    frame(id, style)
        .pl(ui_rems(pad))
        .pr(ui_rems(pad - POPUP_TRAIL_TIGHTEN))
        .justify_start()
        .gap(ui_rems(POPUP_GAP))
}
