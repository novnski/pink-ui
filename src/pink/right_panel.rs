//! Pink's right panel band (#176, D45 to D53; Pink: `shell/right_rail.rs`
//! `render_right_place_header`, `place_segment`, `render_more_button` and
//! `header_action`).
//!
//! The icon rail is gone. The band over the panel names its destinations as
//! labelled segments, `Review` and `Files`, then `More` with its chevron (the
//! current tool, when the user picked one, sits before `More`). The shown
//! destination is semibold on the Secondary fill; the others are medium in
//! muted text with the quiet hover. On the right sit the shown place's own
//! actions as 28 px icon controls: Review carries `History` (History lives
//! under Review), and every place can `Expand` to the whole content area.
//! Overview, Changes and the Terminal place are not destinations any more:
//! the terminal lives in the conversation's drawer.
//!
//! Segments are sized in rems (`ui_rems`), so they scale with the Interface
//! size; the action controls stay fixed like Pink's toolbar controls.

use gpui::{
    AbsoluteLength, AnyElement, Div, FontWeight, IntoElement, MouseButton, Role, SharedString,
    Stateful, div, prelude::*, px, rems, svg,
};

use crate::{
    HoverFill,
    components::{
        icon_button::{IconButtonStyle, icon_button},
        surface_toolbar as st,
    },
    pink::{FillTier, Pink, layout, radius, text_tooltip},
};

const ICON_MORE: &str = "icons/right-panel/alt-arrow-down.svg";
/// Review's History action (Pink `icons::GIT_BRANCH`).
pub const ICON_HISTORY: &str = "icons/right-panel/git-branch.svg";
/// Expand (D49): the panel takes the whole content area.
pub const ICON_EXPAND: &str = "icons/right-panel/maximize.svg";

/// Segment height, side padding, label gap and text, in design points
/// (Pink `place_segment`).
const SEGMENT_HEIGHT: f32 = 26.0;
const SEGMENT_PAD_X: f32 = 7.0;
const SEGMENT_GAP: f32 = 5.0;
const MORE_GAP: f32 = 3.0;
const SEGMENT_TEXT: f32 = 12.5;
const MORE_GLYPH: f32 = 11.0;

/// A destination named in the band. Review and Files are always there; the
/// rest open from More.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Review,
    Files,
}

impl Destination {
    /// The band's fixed destinations, in order (Pink
    /// `RightPlace::DESTINATIONS`).
    pub const ALL: [Destination; 2] = [Destination::Review, Destination::Files];

    pub fn label(self) -> &'static str {
        match self {
            Destination::Review => "Review",
            Destination::Files => "Files",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Destination::Review => "review",
            Destination::Files => "files",
        }
    }
}

/// Pink `crate::typography::ui_rems`: design points at the 16 pt Interface
/// baseline, scaled with the window's rem size.
fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

fn segment_base(id: SharedString, selected: bool, t: &Pink) -> Stateful<Div> {
    div()
        .id(id.clone())
        .h(ui_rems(SEGMENT_HEIGHT))
        .px(ui_rems(SEGMENT_PAD_X))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .rounded(px(radius::SM))
        .cursor_pointer()
        .text_size(ui_rems(SEGMENT_TEXT))
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::MEDIUM
        })
        .text_color(if selected {
            t.text
        } else {
            t.readable_muted(0.85)
        })
        .when(selected, |el| el.bg(t.fill(FillTier::Secondary)))
        .when(!selected, |el| {
            el.hover_wash(id, t.fill(FillTier::Tertiary))
        })
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
}

/// One destination segment: its name, semibold on the Secondary fill while
/// it is shown.
pub fn segment(key: &str, destination: Destination, selected: bool, t: &Pink) -> Stateful<Div> {
    segment_base(
        SharedString::from(format!("{key}-place-{}", destination.key())),
        selected,
        t,
    )
    .role(Role::Tab)
    .aria_label(destination.label())
    .aria_selected(selected)
    .tab_index(0)
    .gap(ui_rems(SEGMENT_GAP))
    .child(destination.label())
}

/// `More` with its chevron: the list of tools (Browser, Work, extension
/// panels). It is a menu trigger, not a destination.
pub fn more(key: &str, t: &Pink) -> Stateful<Div> {
    segment_base(SharedString::from(format!("{key}-place-more")), false, t)
        .role(Role::Button)
        .aria_label("More tools")
        .tab_index(0)
        .gap(ui_rems(MORE_GAP))
        .child("More")
        .child(
            svg()
                .path(ICON_MORE)
                .flex_none()
                .size(ui_rems(MORE_GLYPH))
                .text_color(t.text_muted),
        )
}

/// A header action: an icon control whose name shows on hover, so the band
/// keeps its room for the destinations.
pub fn action(
    id: impl Into<SharedString>,
    glyph: &'static str,
    label: &'static str,
    t: &Pink,
) -> Stateful<Div> {
    let id = id.into();
    icon_button(
        id.clone(),
        st::control_glyph(svg().path(glyph).flex_none(), t.text_muted).into_any_element(),
        &IconButtonStyle {
            size: st::CONTROL_SIZE,
            radius: st::CONTROL_RADIUS,
            wash: gpui::transparent_black(),
            enabled: true,
        },
    )
    .hover_wash(id, t.fill(FillTier::Tertiary))
    .role(Role::Button)
    .aria_label(label)
    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
    .tooltip(text_tooltip(label))
    .tooltip_show_delay(layout::TOOLTIP_DELAY)
}

/// The band: `Review`, `Files` and `More` on the left, `actions` on the
/// right, as tall as the titlebar it sits in.
pub fn band(
    key: &str,
    shown: Destination,
    actions: impl IntoIterator<Item = AnyElement>,
    t: &Pink,
) -> Div {
    let mut segments = div()
        .id(SharedString::from(format!("{key}-places")))
        .role(Role::TabList)
        .aria_label("Right panel")
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(1.0));
    for destination in Destination::ALL {
        segments = segments.child(segment(key, destination, destination == shown, t));
    }
    segments = segments.child(more(key, t));
    div()
        .h(px(layout::TITLEBAR_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .pl(px(2.0))
        .pr(px(st::EDGE_INSET))
        .child(segments)
        .child(div().flex_1().min_w_0())
        .children(actions)
}
