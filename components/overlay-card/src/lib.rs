// PINK's overlay cards (popover.rs `tooltip_card`, `TextTooltip`, the
// above-the-trigger tooltip root, `dialog_card` in `modal`, `ModalWarning`
// and `stack_popup_card`): the opaque card every tooltip and hover card
// draws on, the card a modal dialog lays on its scrim, the warning paint
// inside it, and the full-width popup step above the composer. UI rethink
// decision 54: drawn here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the colours, shadows and font,
// the tooltip's type style, the scrim, the frost, the entrance and where
// each card opens; it adds the ids, the listeners, the focus and the
// accessibility. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{BoxShadow, Div, Hsla, Rems, SharedString, div, prelude::*, px, rems};

/// A tooltip card's corner radius.
pub const TOOLTIP_RADIUS: f32 = 8.0;
/// A text tooltip's horizontal padding, in pt at the 16 px default.
pub const TOOLTIP_PAD_X: f32 = 8.0;
/// A text tooltip's vertical padding, in pt at the 16 px default.
pub const TOOLTIP_PAD_Y: f32 = 5.0;
/// Widest a text tooltip grows before its text wraps, in pt at the 16 px
/// default.
pub const TOOLTIP_MAX_WIDTH: f32 = 260.0;
/// Labels longer than this many characters wrap at `TOOLTIP_MAX_WIDTH`;
/// shorter ones stay on one line.
pub const TOOLTIP_WRAP_CHARS: usize = 48;

/// A dialog card's width, in pt at the 16 px default.
pub const DIALOG_WIDTH: f32 = 360.0;
/// A dialog card's padding, in pt at the 16 px default.
pub const DIALOG_PAD: f32 = 20.0;
/// A dialog card's corner radius; the host's modal frost uses the same.
pub const MODAL_RADIUS: f32 = 16.0;

/// A modal warning's wash: the warning tone at this alpha.
pub const WARNING_WASH: f32 = 0.08;
/// A modal warning's edge: the warning tone at this alpha.
pub const WARNING_BORDER: f32 = 0.2;

/// The full-width popup above the composer: the first step of the radius
/// ladder down from the composer pill's corner.
pub const STACK_POPUP_RADIUS: f32 = 20.0;
/// The full-width popup's inset from its edge to its rows.
pub const STACK_POPUP_INSET: f32 = 6.0;
/// A row's corner inside the full-width popup: concentric across its inset.
pub const STACK_POPUP_ROW_RADIUS: f32 = STACK_POPUP_RADIUS - STACK_POPUP_INSET;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> Rems {
    rems(design / 16.0)
}

/// What a tooltip card draws.
pub struct TooltipStyle {
    pub border: Hsla,
    /// Opaque: a tooltip is never translucent or frosted.
    pub background: Hsla,
    pub shadows: Vec<BoxShadow>,
    /// Named on the card itself: a tooltip lays out as its own root, outside
    /// the window's element tree, and inherits nothing.
    pub font_family: SharedString,
}

/// The card every tooltip and rich hover card draws on: rounded to
/// `TOOLTIP_RADIUS`, a 1 px edge, an opaque fill and a soft shadow. The
/// content sets its own padding and width.
pub fn tooltip_card(style: &TooltipStyle) -> Div {
    div()
        .rounded(px(TOOLTIP_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .shadow(style.shadows.clone())
        .font_family(style.font_family.clone())
}

/// Whether a text tooltip's label wraps at `TOOLTIP_MAX_WIDTH` rather than
/// running on one line.
pub fn tooltip_wraps(label: &str) -> bool {
    label.chars().count() > TOOLTIP_WRAP_CHARS
}

/// The plain-text tooltip: the tooltip card padded `TOOLTIP_PAD_X` by
/// `TOOLTIP_PAD_Y`, the label in `text`, on one line up to
/// `TOOLTIP_WRAP_CHARS` characters and wrapped at `TOOLTIP_MAX_WIDTH` past
/// it. The host chains the caption type style.
pub fn text_tooltip(style: &TooltipStyle, text: Hsla, label: impl Into<SharedString>) -> Div {
    let label = label.into();
    let wraps = tooltip_wraps(&label);
    tooltip_card(style)
        .when(wraps, |card| card.w(ui_rems(TOOLTIP_MAX_WIDTH)))
        .when(!wraps, |card| card.whitespace_nowrap())
        .px(ui_rems(TOOLTIP_PAD_X))
        .py(ui_rems(TOOLTIP_PAD_Y))
        .text_color(text)
        .child(label)
}

/// The root of a tooltip shown above its trigger: zero-sized at the anchor
/// point, so the card it holds overflows upward and to both sides, centred.
/// The host positions the anchor.
pub fn above(card: impl IntoElement) -> Div {
    div()
        .size_0()
        .flex()
        .flex_col()
        .justify_end()
        .items_center()
        .child(div().flex_none().child(card))
}

/// What a dialog card draws.
pub struct DialogStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub text: Hsla,
    /// A large drop shadow under an opaque card. A translucent (frosted)
    /// card leaves it off: the shadow would show through the plate.
    pub lifted: bool,
}

/// The card a modal dialog lays on its scrim: `DIALOG_WIDTH` wide, padded
/// `DIALOG_PAD`, rounded to `MODAL_RADIUS` with a 1 px edge, a column in
/// `text`. The host chains its title, body, fields and buttons.
pub fn dialog_card(style: &DialogStyle) -> Div {
    div()
        .w(ui_rems(DIALOG_WIDTH))
        .p(ui_rems(DIALOG_PAD))
        .rounded(px(MODAL_RADIUS))
        .bg(style.background)
        .border_1()
        .border_color(style.border)
        .when(style.lifted, |card| card.shadow_lg())
        .flex()
        .flex_col()
        .text_color(style.text)
}

/// How a warning notice paints inside a dialog card: a faint wash of the
/// warning tone, a firmer edge, and copy the host has already floored for
/// contrast on that wash over the card and its scrim.
pub struct ModalWarning {
    pub wash: Hsla,
    pub border: Hsla,
    pub text: Hsla,
}

/// The warning wash: `tone` at `WARNING_WASH`.
pub fn warning_wash(tone: Hsla) -> Hsla {
    tone.opacity(WARNING_WASH)
}

/// The warning edge: `tone` at `WARNING_BORDER`.
pub fn warning_border(tone: Hsla) -> Hsla {
    tone.opacity(WARNING_BORDER)
}

/// A warning notice's paint: the wash, a 1 px edge and the copy colour. The
/// host chains its layout, padding, corner and text.
pub fn warning_notice(warning: &ModalWarning) -> Div {
    div()
        .border_1()
        .border_color(warning.border)
        .bg(warning.wash)
        .text_color(warning.text)
}

/// The full-width popup above the composer: the host's menu card on the
/// stack ladder's popup step, rounded to `STACK_POPUP_RADIUS` with its rows
/// `STACK_POPUP_INSET` in.
pub fn stack_popup(card: Div) -> Div {
    card.rounded(px(STACK_POPUP_RADIUS))
        .p(px(STACK_POPUP_INSET))
}
