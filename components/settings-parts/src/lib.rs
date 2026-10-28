// PINK's settings row parts (settings/widgets.rs `row_tile`, `meta_line`,
// `option_card_row`, `option_card`, `error_strip` and `warning_strip`): the
// identity tile at a row's leading edge, the quiet facts line under a row
// title, the row of preview cards for picking a visual option, and the
// danger and caution strips a card shows above or between its rows. UI
// rethink decision 54: drawn here, consumed by PINK through a pinned
// revision, beside `pink-settings-section`'s card and rows.
//
// The package draws only. The host supplies the resolved colours (the
// readable tones, the strip plates, the selection accent), the icon paths
// and the option previews, and adds the ids, roles, accessible names, focus
// rings and listeners. Every size next to text is a rem, so a page follows
// the host's Interface size; hairlines, corners and the selection ring stay
// in pixels. Plain `//` comments: the pink-ui gallery compiles this file in
// place with `include!` for its story.

use gpui::{
    AbsoluteLength, AnyElement, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems,
    svg,
};

/// A row tile's side.
pub const TILE_SIZE: f32 = 28.0;
/// A row tile's corner radius, in pixels.
pub const TILE_RADIUS: f32 = 7.0;
/// The icon inside a row tile.
pub const TILE_ICON: f32 = 16.0;

/// The space between a row title and its meta line.
pub const META_TOP: f32 = 1.0;
/// The horizontal gap between meta line fragments.
pub const META_GAP_X: f32 = 12.0;
/// The vertical gap when a meta line wraps.
pub const META_GAP_Y: f32 = 2.0;
/// A meta line's text size (the row description's).
pub const META_TEXT_SIZE: f32 = 12.0;

/// The gap between the cards in an option card row.
pub const OPTION_ROW_GAP: f32 = 16.0;
/// An option card's preview frame height.
pub const OPTION_CARD_HEIGHT: f32 = 148.0;
/// The preview frame's corner radius, in pixels. A preview that paints its
/// own background must round itself to this: GPUI clips to the frame's
/// bounding box, not its corners.
pub const OPTION_CARD_RADIUS: f32 = 6.0;
/// The gap between the preview frame and the caption.
pub const OPTION_CAPTION_GAP: f32 = 8.0;
/// The gap between the caption glyph and label.
pub const OPTION_GLYPH_GAP: f32 = 6.0;
/// The caption glyph's size.
pub const OPTION_GLYPH: f32 = 16.0;
/// The caption label's size.
pub const OPTION_LABEL_SIZE: f32 = 13.0;
/// The selection ring's stroke and its gap to the frame, in pixels.
pub const RING_WIDTH: f32 = 2.0;
pub const RING_GAP: f32 = 2.0;

/// A strip's horizontal padding.
pub const STRIP_PAD_X: f32 = 16.0;
/// A strip's corner radius, in pixels.
pub const STRIP_RADIUS: f32 = 12.0;
/// The gap between a strip's icon and its message.
pub const STRIP_GAP: f32 = 8.0;
/// The icon's drop from the strip's first line.
pub const STRIP_ICON_TOP: f32 = 2.0;
/// The error strip: its space above, vertical padding, text and icon size.
pub const ERROR_STRIP_TOP: f32 = 16.0;
pub const ERROR_STRIP_PAD_Y: f32 = 12.0;
pub const ERROR_STRIP_TEXT: f32 = 12.5;
pub const ERROR_STRIP_ICON: f32 = 16.0;
/// The warning strip: its space above, vertical padding, text and icon size.
pub const WARNING_STRIP_TOP: f32 = 8.0;
pub const WARNING_STRIP_PAD_Y: f32 = 10.0;
pub const WARNING_STRIP_TEXT: f32 = 12.0;
pub const WARNING_STRIP_ICON: f32 = 14.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a row tile draws.
pub struct TileStyle {
    pub border: Hsla,
    /// The tile's quiet fill.
    pub background: Hsla,
    pub icon: Hsla,
}

/// A row's identity tile: `TILE_SIZE` square, `TILE_RADIUS` corners, a 1 px
/// border, a quiet fill and a centred `TILE_ICON` icon. Callers may add a
/// status dot as a child (it is a plain `Div`).
pub fn row_tile(style: &TileStyle, icon_path: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .size(ui_rems(TILE_SIZE))
        .rounded(px(TILE_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(icon_path)
                .flex_none()
                .size(ui_rems(TILE_ICON))
                .text_color(style.icon),
        )
}

/// The quiet line under a row title. Fragments are separated by space,
/// never by a glyph (UI rethink decision 31): each fragment is its own fact
/// and a screen reader reads them as separate runs. A fragment may wrap but
/// never overflows into the control column: it can shrink below its
/// one-line width.
pub fn meta_line(color: Hsla, fragments: impl IntoIterator<Item = AnyElement>) -> Div {
    let mut line = div()
        .min_w_0()
        .mt(ui_rems(META_TOP))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(ui_rems(META_GAP_X))
        .gap_y(ui_rems(META_GAP_Y))
        .text_size(ui_rems(META_TEXT_SIZE))
        .text_color(color);
    for fragment in fragments {
        // GPUI's default flex shrink is 1, so `min_w_0` alone lets the
        // fragment shrink (crates.io GPUI's `flex_shrink` takes no factor).
        line = line.child(div().min_w_0().child(fragment));
    }
    line
}

/// What an option card draws.
pub struct OptionCardStyle {
    /// The selection ring (the accent).
    pub ring: Hsla,
    /// The preview frame's 1 px edge.
    pub border: Hsla,
    /// The caption and its glyph when the card is chosen.
    pub text: Hsla,
    /// The caption and its glyph otherwise.
    pub text_muted: Hsla,
}

/// A row of equally sized option cards, `OPTION_ROW_GAP` apart.
pub fn option_card_row() -> Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(ui_rems(OPTION_ROW_GAP))
        .w_full()
}

/// The selection ring around a surface with `inner_radius` corners: a
/// `RING_WIDTH` stroke `RING_GAP` outside it, in `color` when `selected`
/// and transparent otherwise, so choosing never moves the surface.
pub fn selection_ring(
    selected: bool,
    inner_radius: f32,
    color: Hsla,
    child: impl IntoElement,
) -> Div {
    div()
        .flex_none()
        .p(px(RING_GAP))
        // RING_WIDTH.
        .border_2()
        .border_color(if selected {
            color
        } else {
            gpui::transparent_black()
        })
        .rounded(px(inner_radius + RING_GAP + RING_WIDTH))
        .child(child)
}

/// One card in an `option_card_row`: an `OPTION_CARD_HEIGHT` preview frame
/// (`OPTION_CARD_RADIUS`, 1 px border, clipped) inside the selection ring,
/// and a caption under it: the glyph at `icon_path` and the label, medium
/// `text` when chosen, regular `text_muted` otherwise. The glyph takes the
/// label's colour: an svg paints only with its own text colour, never an
/// inherited one, so an uncoloured glyph would leave a blank gap. The accent
/// is reserved for the ring. The caller adds the id, role, focus and click.
pub fn option_card(
    style: &OptionCardStyle,
    icon_path: impl Into<SharedString>,
    label: impl Into<SharedString>,
    selected: bool,
    preview: impl IntoElement,
) -> Div {
    let caption = if selected {
        style.text
    } else {
        style.text_muted
    };
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(ui_rems(OPTION_CAPTION_GAP))
        .cursor_pointer()
        .child(
            selection_ring(
                selected,
                OPTION_CARD_RADIUS,
                style.ring,
                div()
                    .h(ui_rems(OPTION_CARD_HEIGHT))
                    .w_full()
                    .rounded(px(OPTION_CARD_RADIUS))
                    .overflow_hidden()
                    .border_1()
                    .border_color(style.border)
                    .child(preview),
            )
            .w_full(),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(ui_rems(OPTION_GLYPH_GAP))
                .text_size(ui_rems(OPTION_LABEL_SIZE))
                .font_weight(if selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(caption)
                .child(
                    svg()
                        .path(icon_path)
                        .flex_none()
                        .size(ui_rems(OPTION_GLYPH))
                        .text_color(caption),
                )
                .child(label.into()),
        )
}

/// What an error or warning strip draws.
pub struct StripStyle {
    /// The 1 px edge (the tone at 20%).
    pub edge: Hsla,
    /// The plate (the tone at 6%).
    pub plate: Hsla,
    /// The message and icon (the readable muted tone).
    pub tone: Hsla,
}

/// The red error strip: `ERROR_STRIP_TOP` above it, `STRIP_PAD_X` by
/// `ERROR_STRIP_PAD_Y`, `STRIP_RADIUS` corners, a 1 px edge on its plate,
/// and the `ERROR_STRIP_ICON` icon at `icon_path` before the wrapping
/// `ERROR_STRIP_TEXT` message.
pub fn error_strip(
    style: &StripStyle,
    icon_path: impl Into<SharedString>,
    message: impl Into<SharedString>,
) -> Div {
    strip(
        style,
        icon_path.into(),
        message.into(),
        ERROR_STRIP_TOP,
        ERROR_STRIP_PAD_Y,
        ERROR_STRIP_TEXT,
        ERROR_STRIP_ICON,
    )
}

/// The amber warning strip: the error strip's anatomy, closer to what it
/// follows (`WARNING_STRIP_TOP`), slimmer (`WARNING_STRIP_PAD_Y`), with
/// `WARNING_STRIP_TEXT` copy and a `WARNING_STRIP_ICON` icon.
pub fn warning_strip(
    style: &StripStyle,
    icon_path: impl Into<SharedString>,
    message: impl Into<SharedString>,
) -> Div {
    strip(
        style,
        icon_path.into(),
        message.into(),
        WARNING_STRIP_TOP,
        WARNING_STRIP_PAD_Y,
        WARNING_STRIP_TEXT,
        WARNING_STRIP_ICON,
    )
}

fn strip(
    style: &StripStyle,
    icon_path: SharedString,
    message: SharedString,
    top: f32,
    pad_y: f32,
    text: f32,
    icon: f32,
) -> Div {
    div()
        .mt(ui_rems(top))
        .px(ui_rems(STRIP_PAD_X))
        .py(ui_rems(pad_y))
        .rounded(px(STRIP_RADIUS))
        .border_1()
        .border_color(style.edge)
        .bg(style.plate)
        .text_size(ui_rems(text))
        .text_color(style.tone)
        .flex()
        .flex_row()
        .items_start()
        .gap(ui_rems(STRIP_GAP))
        .child(
            div().flex_none().mt(ui_rems(STRIP_ICON_TOP)).child(
                svg()
                    .path(icon_path)
                    .flex_none()
                    .size(ui_rems(icon))
                    .text_color(style.tone),
            ),
        )
        .child(div().min_w_0().child(message))
}
