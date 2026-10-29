// PINK's floating card, its rows and the chrome around them (popover.rs
// `popover_card`, `menu_row`, `menu_heading`, `menu_separator`,
// `menu_section`, `search_input_frame`, `palette_card`,
// `palette_search_icon` and the key hints `key_cap`, `key_hint`,
// `key_hint_text`, `key_hint_pair`, `kbd_hint`): the surface every menu,
// picker and popover opens on, the one row shape its items share, the
// headings, rules and search field that divide and lead them, and the key
// legends in their rows and footers. UI rethink decision 54: drawn here,
// consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the colours, the card's shadow
// (none on a frosted surface), the fonts, the glyph paths and each row's
// text and fill already blended over its hover fade; it adds the ids, the
// hover listeners, the actions, the text inputs and the accessibility.
// Plain `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{
    AbsoluteLength, BoxShadow, Div, FontWeight, Hsla, Pixels, SharedString, div, prelude::*, px,
    rems, svg,
};

/// The card's corner radius.
pub const CARD_RADIUS: f32 = 12.0;
/// The card's inset from its edge to its rows.
pub const CARD_INSET: f32 = 4.0;
/// The gap between rows.
pub const MENU_GAP: f32 = 2.0;
/// A row's corner radius: concentric with the card across its inset.
pub const ITEM_RADIUS: f32 = CARD_RADIUS - CARD_INSET;
/// Text size in the card and its rows, in pt at the 16 px default.
pub const TEXT: f32 = 13.0;
/// A row's gap between its parts, in pt at the 16 px default.
pub const ROW_GAP: f32 = 10.0;
/// A row's horizontal padding, in pt at the 16 px default.
pub const ROW_PAD_X: f32 = 8.0;
/// A row's vertical padding, in pt at the 16 px default.
pub const ROW_PAD_Y: f32 = 6.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the card draws.
pub struct CardStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub text: Hsla,
    pub font_family: SharedString,
    /// The card's lift; `None` on a surface that carries its own depth.
    pub shadows: Option<Vec<BoxShadow>>,
}

/// The floating card: a bordered, rounded surface with its rows stacked
/// inside `CARD_INSET`. The host chains its layout, size and children.
pub fn card(style: &CardStyle) -> Div {
    let card = div()
        .border_1()
        .border_color(style.border)
        .rounded(px(CARD_RADIUS));
    let card = match &style.shadows {
        Some(shadows) => card.shadow(shadows.clone()),
        None => card,
    };
    card.bg(style.background)
        .p(px(CARD_INSET))
        .gap(px(MENU_GAP))
        .overflow_hidden()
        .text_size(ui_rems(TEXT))
        .font_family(style.font_family.clone())
        .text_color(style.text)
}

/// What one row draws: its text colour and fill, each already resolved for
/// the selected state or blended over the hover fade.
pub struct RowStyle {
    pub text: Hsla,
    pub fill: Hsla,
}

/// One row: its parts in a line, padded and rounded to sit concentric in the
/// card. The host adds the id, the hover listener and the action.
pub fn row(style: &RowStyle) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .px(ui_rems(ROW_PAD_X))
        .py(ui_rems(ROW_PAD_Y))
        .rounded(px(ITEM_RADIUS))
        .text_size(ui_rems(TEXT))
        .cursor_pointer()
        .bg(style.fill)
        .text_color(style.text)
}

// ---------------------------------------------------------------------------
// Headings, rules and sections
// ---------------------------------------------------------------------------

/// A heading's horizontal padding, in pt at the 16 px default.
pub const HEADING_PAD_X: f32 = 8.0;
/// A heading's padding above its label, in pt at the 16 px default.
pub const HEADING_PAD_TOP: f32 = 6.0;
/// A heading's padding below its label, in pt at the 16 px default.
pub const HEADING_PAD_BOTTOM: f32 = 4.0;
/// A heading's text size, in pt at the 16 px default: the menu type ramp's
/// floor.
pub const HEADING_TEXT: f32 = 11.0;
/// A trailing section's margin and padding above its rule, in pt at the
/// 16 px default.
pub const SECTION_SPACE: f32 = 4.0;

/// A small uppercase heading over a group of rows: `label` uppercased and
/// tracked (`tracked_upper`), semibold at `HEADING_TEXT` in `color`, padded
/// `HEADING_PAD_X` with `HEADING_PAD_TOP` above and `HEADING_PAD_BOTTOM`
/// below.
pub fn heading(color: Hsla, label: &str) -> Div {
    div()
        .px(ui_rems(HEADING_PAD_X))
        .pb(ui_rems(HEADING_PAD_BOTTOM))
        .pt(ui_rems(HEADING_PAD_TOP))
        .text_size(ui_rems(HEADING_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(SharedString::from(tracked_upper(label)))
}

/// `label` uppercased with a hair space (U+200A) between its characters,
/// about 0.1em of tracking: GPUI has no letter spacing.
pub fn tracked_upper(label: &str) -> String {
    let upper = label.to_uppercase();
    let mut out = String::with_capacity(upper.len() * 2);
    for (ix, ch) in upper.chars().enumerate() {
        if ix > 0 {
            out.push('\u{200A}');
        }
        out.push(ch);
    }
    out
}

/// The 1 px rule between groups of rows in `color`, `MENU_GAP` above and
/// below. It runs border to border: negative margins cancel the card's
/// `CARD_INSET`.
pub fn separator(color: Hsla) -> Div {
    div()
        .h(px(1.0))
        .mx(px(-CARD_INSET))
        .my(px(MENU_GAP))
        .bg(color)
}

/// A trailing block of rows under a 1 px top rule in `rule`, set
/// `SECTION_SPACE` below the rows before it and `SECTION_SPACE` above its
/// own, a column `MENU_GAP` apart. The rule spans the card's inner width.
pub fn section(rule: Hsla) -> Div {
    div()
        .mt(ui_rems(SECTION_SPACE))
        .pt(ui_rems(SECTION_SPACE))
        .border_t_1()
        .border_color(rule)
        .flex()
        .flex_col()
        .gap(px(MENU_GAP))
}

// ---------------------------------------------------------------------------
// Search field and command palette
// ---------------------------------------------------------------------------

/// The search field's horizontal padding, in pt at the 16 px default.
pub const SEARCH_PAD_X: f32 = 10.0;
/// The search field's vertical padding, in pt at the 16 px default.
pub const SEARCH_PAD_Y: f32 = 6.0;
/// The gap under the search field, in pt at the 16 px default.
pub const SEARCH_MARGIN: f32 = 4.0;
/// The search field's corner radius.
pub const SEARCH_RADIUS: f32 = 8.0;
/// The search field's text size, in pt at the 16 px default.
pub const SEARCH_TEXT: f32 = TEXT;
/// The palette's search glyph and its slot, in pt at the 16 px default: the
/// same slot as a palette row's glyph.
pub const SEARCH_GLYPH: f32 = 16.0;

/// The borderless search field at the top of a picker: full width inside
/// the card's inset on a `fill` plate rounded to `SEARCH_RADIUS`, padded
/// `SEARCH_PAD_X` by `SEARCH_PAD_Y`, `SEARCH_MARGIN` above the rows. The host
/// adds the text input as its child.
pub fn search_frame(fill: Hsla) -> Div {
    div()
        .mb(ui_rems(SEARCH_MARGIN))
        .px(ui_rems(SEARCH_PAD_X))
        .py(ui_rems(SEARCH_PAD_Y))
        .rounded(px(SEARCH_RADIUS))
        .bg(fill)
        .text_size(ui_rems(SEARCH_TEXT))
}

/// The palette's search glyph: the `glyph` path at `SEARCH_GLYPH` in
/// `color`, centred in its own `SEARCH_GLYPH` slot.
pub fn search_glyph(glyph: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .size(ui_rems(SEARCH_GLYPH))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(glyph)
                .flex_none()
                .size(ui_rems(SEARCH_GLYPH))
                .text_color(color),
        )
}

/// What a command palette's card draws.
pub struct PaletteStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub text: Hsla,
    pub font_family: SharedString,
}

/// The card a command palette and its sibling flows open on: `width` wide,
/// rounded to `corner_radius` with a 1 px edge, a large drop shadow, clipped,
/// a column. The host supplies its recessed header and footer bands.
pub fn palette_card(style: &PaletteStyle, width: Pixels, corner_radius: f32) -> Div {
    div()
        .w(width)
        .rounded(px(corner_radius))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .shadow_lg()
        .overflow_hidden()
        .flex()
        .flex_col()
        .font_family(style.font_family.clone())
        .text_color(style.text)
}

// ---------------------------------------------------------------------------
// Key hints
// ---------------------------------------------------------------------------

/// A footer key cap's height, in pt at the 16 px default.
pub const KEY_CAP_HEIGHT: f32 = 22.0;
/// A key cap's (and an inline key hint's) horizontal padding, in pt at the
/// 16 px default.
pub const KEY_PAD_X: f32 = 5.0;
/// An inline key hint's vertical padding, in pt at the 16 px default.
pub const KBD_PAD_Y: f32 = 1.0;
/// A key cap's corner radius.
pub const KEY_RADIUS: f32 = 5.0;
/// The gap between the parts on a key cap, in pt at the 16 px default.
pub const KEY_CAP_GAP: f32 = 4.0;
/// The gap between a key cap and its verb, in pt at the 16 px default.
pub const KEY_HINT_GAP: f32 = 5.0;
/// A glyph on a key cap, in pt at the 16 px default.
pub const KEY_GLYPH: f32 = 12.5;
/// Key words and verbs, in pt at the 16 px default.
pub const KEY_TEXT: f32 = 11.0;

/// What a key hint draws.
pub struct KeyHintStyle {
    /// The key cap's plate.
    pub cap: Hsla,
    /// The glyphs, the key words and the verb.
    pub text: Hsla,
    /// The hairline between a pair's two glyphs.
    pub divider: Hsla,
    /// The face of a key word.
    pub mono: SharedString,
}

/// One footer key cap holding any children: `KEY_CAP_HEIGHT` tall, padded
/// `KEY_PAD_X`, rounded to `KEY_RADIUS` on a `fill` plate, its parts centred
/// in a row `KEY_CAP_GAP` apart.
pub fn key_cap(fill: Hsla) -> Div {
    div()
        .h(ui_rems(KEY_CAP_HEIGHT))
        .px(ui_rems(KEY_PAD_X))
        .rounded(px(KEY_RADIUS))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(ui_rems(KEY_CAP_GAP))
        .bg(fill)
}

/// A glyph on a key cap: the `path` at `KEY_GLYPH` in `color`.
fn key_glyph(path: SharedString, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .flex_none()
        .size(ui_rems(KEY_GLYPH))
        .text_color(color)
}

/// The verb after a key cap, at `KEY_TEXT`.
fn key_verb(color: Hsla, label: SharedString) -> Div {
    div()
        .text_size(ui_rems(KEY_TEXT))
        .text_color(color)
        .child(label)
}

/// A legend: `cap` and its verb in a row `KEY_HINT_GAP` apart.
fn legend(cap: Div, style: &KeyHintStyle, label: SharedString) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(KEY_HINT_GAP))
        .child(cap)
        .child(key_verb(style.text, label))
}

/// A footer legend: a key cap holding one `glyph`, then its verb.
pub fn key_hint(
    style: &KeyHintStyle,
    glyph: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Div {
    legend(
        key_cap(style.cap).child(key_glyph(glyph.into(), style.text)),
        style,
        label.into(),
    )
}

/// A footer legend whose cap holds a word ("tab", "esc") in the mono face at
/// `KEY_TEXT`, for keys with no glyph.
pub fn key_hint_text(
    style: &KeyHintStyle,
    cap: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Div {
    legend(
        key_cap(style.cap)
            .text_size(ui_rems(KEY_TEXT))
            .font_family(style.mono.clone())
            .text_color(style.text)
            .child(cap.into()),
        style,
        label.into(),
    )
}

/// A footer legend whose cap holds two glyphs split by a 1 px hairline
/// ("↑ | ↓ Navigate"), sharing one verb.
pub fn key_hint_pair(
    style: &KeyHintStyle,
    first: impl Into<SharedString>,
    second: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Div {
    legend(
        key_cap(style.cap)
            .child(key_glyph(first.into(), style.text))
            .child(div().w(px(1.0)).h(ui_rems(KEY_TEXT)).bg(style.divider))
            .child(key_glyph(second.into(), style.text)),
        style,
        label.into(),
    )
}

/// A muted accelerator chip inside a row ("⌘↵"): the keys in the mono face at
/// `KEY_TEXT`, padded `KEY_PAD_X` by `KBD_PAD_Y` on the cap plate.
pub fn kbd_hint(style: &KeyHintStyle, keys: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(ui_rems(KEY_PAD_X))
        .py(ui_rems(KBD_PAD_Y))
        .rounded(px(KEY_RADIUS))
        .bg(style.cap)
        .text_size(ui_rems(KEY_TEXT))
        .font_family(style.mono.clone())
        .text_color(style.text)
        .child(keys.into())
}
