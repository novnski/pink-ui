// PINK's Integration Picker (Settings → Extensions, issue #374): the page of
// integrations, with a search in the title bar, a Connected and an Available
// section card of service rows, and the enable review that gives each
// permission its own switch. PINK's settings scaffolding (the section card,
// card row, row text, meta line and badges) comes from
// `pink-settings-parts`; this package draws the picker's own pieces: the
// title-bar search field, the monogram identity tile, the mono identity line
// in the review, and the review's permission rows. It also owns the picker's
// rule for which section a row belongs to and what a search matches, so the
// gallery and PINK filter the same way.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours, fonts, the
// icon path, the text input and every word; it adds the ids, roles and
// accessible names, focus, keyboard, hover fills and the actions. Plain `//`
// comments: the pink-ui gallery compiles this file in place with `include!`
// for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, rems, svg};

// The title bar's search field (PINK's `pink_surface_toolbar::field`), in pt
// at the 16 px default: `SEARCH_W` wide and as tall as the title bar's
// actions.
pub const SEARCH_W: f32 = 220.0;
pub const SEARCH_H: f32 = 28.0;
pub const SEARCH_RADIUS: f32 = 6.0;
pub const SEARCH_PAD_X: f32 = 8.0;
pub const SEARCH_GAP: f32 = 6.0;
pub const SEARCH_ICON: f32 = 12.0;
pub const SEARCH_TEXT: f32 = 12.0;

// The row's identity tile: PINK's 28 row tile, radius 7, carrying the
// integration's initial instead of an icon.
pub const TILE: f32 = 28.0;
pub const TILE_RADIUS: f32 = 7.0;
pub const MONOGRAM_TEXT: f32 = 14.0;
pub const MONOGRAM_LINE: f32 = 16.0;

// The review dialog: `REVIEW_W` wide; the identity line `IDENTITY_TOP` under
// the body in mono `IDENTITY_TEXT`; the permissions `PERMISSIONS_TOP` below,
// `PERMISSION_GAP` apart, each with `PERMISSION_PART_GAP` between its text
// and its switch.
pub const REVIEW_W: f32 = 460.0;
pub const IDENTITY_TOP: f32 = 6.0;
pub const IDENTITY_TEXT: f32 = 11.0;
pub const PERMISSIONS_TOP: f32 = 20.0;
pub const PERMISSION_GAP: f32 = 10.0;
pub const PERMISSION_PART_GAP: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The two section cards, Connected first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Connected,
    Available,
}

impl Section {
    pub fn title(self) -> &'static str {
        match self {
            Section::Connected => "Connected",
            Section::Available => "Available",
        }
    }
}

/// Whether a search matches one of `fields`, case-insensitively. An empty or
/// blank query matches everything.
pub fn matches(query: &str, fields: &[&str]) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || fields
            .iter()
            .any(|field| field.to_lowercase().contains(&query))
}

/// The visible rows as `(section, rows)`, Connected first, each in the order
/// given. A section with no matching row is left out, so an empty result
/// means the search matched nothing.
pub fn arrange<'a, T>(
    items: &'a [T],
    query: &str,
    connected: impl Fn(&T) -> bool,
    fields: impl Fn(&T) -> Vec<&str>,
) -> Vec<(Section, Vec<&'a T>)> {
    let (on, off): (Vec<&T>, Vec<&T>) = items
        .iter()
        .filter(|item| matches(query, &fields(item)))
        .partition(|item| connected(item));
    [(Section::Connected, on), (Section::Available, off)]
        .into_iter()
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

/// The first letter or digit of `name`, upper-cased; empty when it has none.
pub fn monogram(name: &str) -> SharedString {
    name.chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_default()
        .into()
}

/// The search field's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct SearchStyle {
    pub fill: Hsla,
    pub glyph: Hsla,
}

/// The title bar's search field: the magnifier glyph and the host's text
/// input on a `SEARCH_W` × `SEARCH_H` field. The host supplies the input
/// (with its placeholder, `Role::SearchInput` and accessible name) and the
/// keyboard handling.
pub fn search_field(
    style: &SearchStyle,
    icon: impl Into<SharedString>,
    input: impl IntoElement,
) -> Div {
    div()
        .flex_none()
        .w(ui_rems(SEARCH_W))
        .h(ui_rems(SEARCH_H))
        .px(ui_rems(SEARCH_PAD_X))
        .rounded(ui_rems(SEARCH_RADIUS))
        .bg(style.fill)
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(SEARCH_GAP))
        .text_size(ui_rems(SEARCH_TEXT))
        .child(
            svg()
                .path(icon.into())
                .flex_none()
                .size(ui_rems(SEARCH_ICON))
                .text_color(style.glyph),
        )
        .child(div().flex_1().min_w_0().child(input))
}

/// The identity tile's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct TileStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub text: Hsla,
}

/// A row's identity tile: the integration's initial (see `monogram`),
/// semibold, on PINK's 28 row tile.
pub fn monogram_tile(style: &TileStyle, name: &str) -> Div {
    div()
        .flex_none()
        .size(ui_rems(TILE))
        .rounded(ui_rems(TILE_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .flex()
        .items_center()
        .justify_center()
        .text_size(ui_rems(MONOGRAM_TEXT))
        .line_height(ui_rems(MONOGRAM_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(style.text)
        .child(monogram(name))
}

/// The review's identity line under its body: the exact thing being
/// enabled (a package hash, a domain) in the host's mono `font`.
pub fn identity_line(
    font: impl Into<SharedString>,
    color: Hsla,
    text: impl Into<SharedString>,
) -> Div {
    div()
        .mt(ui_rems(IDENTITY_TOP))
        .font_family(font.into())
        .text_size(ui_rems(IDENTITY_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The column of permission rows in the review.
pub fn permission_list() -> Div {
    div()
        .mt(ui_rems(PERMISSIONS_TOP))
        .flex()
        .flex_col()
        .gap(ui_rems(PERMISSION_GAP))
}

/// One permission: the host's row text (the permission's label over its
/// detail) and its switch, which the host locks on for a required one.
pub fn permission_row(text: impl IntoElement, switch: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(PERMISSION_PART_GAP))
        .child(text)
        .child(switch)
}

/// A permission's detail line, saying so when it is required.
pub fn permission_detail(detail: &str, required: bool) -> String {
    match (detail.is_empty(), required) {
        (_, false) => detail.to_owned(),
        (true, true) => "Required".to_owned(),
        (false, true) => format!("{detail}, required"),
    }
}
