// PINK's trigger chip (pickers.rs `trigger_chip`): the composer controls
// row's quiet chips, model, access and the like. A ghost pill with no
// border or caret: an icon (with an optional corner dot), the label, a
// badge after it and a muted suffix, on the host's hover wash. Square and
// label-less when the row has no room. UI rethink decision 54: drawn here,
// consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the icon element (its own
// assets, or a spinner while loading), the label (text, or its own
// skeleton), the badge glyph and the colours already blended over its hover
// fade, and adds what crates.io GPUI 0.2.2 can't express (role, accessible
// name and description, focus and its ring), the tooltip and the behaviour
// (press, click, keyboard).

use gpui::{
    AbsoluteLength, AnyElement, ElementId, FontWeight, Hsla, SharedString, Stateful, div,
    prelude::*, px, rems,
};

/// The chip's height, and the icon-only chip's side.
pub const HEIGHT: f32 = 28.0;
/// Horizontal padding of a labelled chip.
pub const PAD_X: f32 = 10.0;
/// Gap between the icon, label, badge and suffix.
pub const GAP: f32 = 6.0;
/// The icon's side.
pub const ICON: f32 = 16.0;
/// The badge's side (the fast-speed bolt).
pub const BADGE: f32 = 13.0;
/// The label's type size.
pub const TEXT: f32 = 12.0;
/// The corner dot's diameter.
pub const DOT: f32 = 6.0;
/// The widest a labelled chip grows before its label truncates.
pub const MAX_WIDTH: f32 = 280.0;

/// How the chip's design sizes (in pt at the 16 px default) become lengths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Units {
    /// Rems, so the chip grows with the host's Interface size (PINK).
    Rems,
    /// Fixed pixels.
    Pixels,
}

impl Units {
    /// A design size as a length in these units.
    pub fn len(self, design: f32) -> AbsoluteLength {
        match self {
            Units::Rems => rems(design / 16.0).into(),
            Units::Pixels => px(design).into(),
        }
    }
}

/// The colours one chip draws, already resolved by the host.
pub struct TriggerChipStyle {
    pub units: Units,
    /// The label and badge colour.
    pub label: Hsla,
    /// The suffix colour.
    pub suffix: Hsla,
    /// The wash (transparent at rest, the hover or open fill otherwise).
    pub wash: Hsla,
}

/// What one chip shows.
#[derive(Default)]
pub struct TriggerChipContent {
    /// The leading icon, or the host's spinner while it loads.
    pub icon: Option<AnyElement>,
    /// The corner dot on the icon (a change still to apply, a choice still
    /// to make).
    pub attention: Option<Hsla>,
    /// The label; ignored when `icon_only`.
    pub label: Option<ChipLabel>,
    /// A mark after the label (the fast-speed bolt); ignored when
    /// `icon_only`.
    pub badge: Option<AnyElement>,
    /// A muted second half after the label ("Medium"); ignored when
    /// `icon_only`.
    pub suffix: Option<SharedString>,
    /// Square and label-less; the host's tooltip names it.
    pub icon_only: bool,
}

/// A chip's label.
pub enum ChipLabel {
    /// Text that truncates when the row is short of room.
    Text(SharedString),
    /// The host's own element (a loading skeleton).
    Element(AnyElement),
}

/// A trigger chip. The host chains its semantics, tooltip, focus ring and
/// actions on the returned element. The chip carries its id as a debug
/// selector for geometry checks.
pub fn trigger_chip(
    id: impl Into<SharedString>,
    content: TriggerChipContent,
    style: &TriggerChipStyle,
) -> Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let selector = id.to_string();
    let len = |design| style.units.len(design);
    let icon_only = content.icon_only;
    div()
        .id(ElementId::Name(id))
        .debug_selector(move || selector)
        .h(len(HEIGHT))
        // Shrinkable under row pressure: the host drops whole pieces first,
        // so truncation is only the last resort for a very narrow row.
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .when(icon_only, |chip| {
            chip.w(len(HEIGHT)).flex_none().justify_center()
        })
        .when(!icon_only, |chip| {
            chip.max_w(len(MAX_WIDTH)).gap(len(GAP)).px(len(PAD_X))
        })
        .rounded_full()
        .text_size(len(TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.label)
        .bg(style.wash)
        .cursor_pointer()
        .when_some(content.icon, |chip, icon| {
            chip.child(div().flex_none().relative().child(icon).when_some(
                content.attention,
                |icon, color| {
                    icon.child(
                        div()
                            .absolute()
                            .top(px(-1.0))
                            .right(px(-2.0))
                            .size(len(DOT))
                            .rounded_full()
                            .bg(color),
                    )
                },
            ))
        })
        .when(!icon_only, |chip| {
            chip.when_some(content.label, |chip, label| match label {
                ChipLabel::Text(text) => chip.child(div().min_w_0().truncate().child(text)),
                ChipLabel::Element(element) => chip.child(element),
            })
            .when_some(content.badge, |chip, badge| chip.child(badge))
            .when_some(content.suffix, |chip, suffix| {
                chip.child(div().flex_none().text_color(style.suffix).child(suffix))
            })
        })
}
