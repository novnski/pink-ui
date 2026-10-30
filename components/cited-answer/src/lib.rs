// PINK's cited answers (issue #371): a web-research reply beside the sources
// it rests on. Superscript citation numbers end the reply spans a provider
// says a source supports; the sources sit in a column of chip cards under a
// "Sources" heading, each numbered as the answer cites it (or marked with a
// globe when the provider shared only a turn-level list, with no spans to
// number). A card folds open on the quoted passage, and pointing at a
// citation previews its source in a hover card.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours (hover and
// lit states already blended), the icon paths and every word; it adds the
// ids, roles and accessible names, focus, keyboard, tooltips, hover state and
// the actions. Plain `//` comments: the pink-ui gallery compiles this file in
// place with `include!` for its story.

use std::sync::Arc;

use gpui::{
    AbsoluteLength, Div, Font, FontFeatures, FontWeight, Hsla, IntoElement, Radians,
    SharedString, Transformation, div, prelude::*, rems, svg,
};

// The column heading (Activity's `activity_section_heading`): a 28 pt row,
// 8 pt in, the 11 pt semibold name with its count trailing.
pub const HEADING_H: f32 = 28.0;
pub const HEADING_PAD_X: f32 = 8.0;
pub const HEADING_TEXT: f32 = 11.0;

// A source card (the transcript's expandable chip card): r10, `CARD_GAP`
// apart in the column, with a two-line `HEADER_H` header.
pub const CARD_RADIUS: f32 = 10.0;
pub const CARD_GAP: f32 = 8.0;
pub const HEADER_H: f32 = 44.0;
pub const HEADER_PAD_X: f32 = 8.0;
pub const HEADER_GAP: f32 = 8.0;
// The header's leading tile (citation number or globe) and trailing chevron.
pub const TILE: f32 = 18.0;
pub const TILE_RADIUS: f32 = 6.0;
pub const TILE_TEXT: f32 = 11.0;
pub const TILE_ICON: f32 = 12.0;
pub const CHEVRON: f32 = 12.0;
// The title over its site line (the command palette history row).
pub const TITLE_TEXT: f32 = 12.0;
pub const TITLE_LINE: f32 = 16.0;
pub const META_TEXT: f32 = 11.0;
pub const META_LINE: f32 = 14.0;
pub const LINE_GAP: f32 = 2.0;

// The folded body: a 1 pt rule, then the passage in the transcript's
// blockquote (a 2 pt rail, right corners 6) at the tool-detail 12/18 size.
pub const QUOTE_INSET: f32 = 8.0;
pub const QUOTE_RAIL: f32 = 2.0;
pub const QUOTE_RADIUS: f32 = 6.0;
pub const QUOTE_PAD_L: f32 = 12.0;
pub const QUOTE_PAD_R: f32 = 10.0;
pub const QUOTE_PAD_Y: f32 = 6.0;
pub const QUOTE_TEXT: f32 = 12.0;
pub const QUOTE_LINE: f32 = 18.0;

// The citation preview (the quoted-passage hover card): the host's
// `PREVIEW_W` popover, `PREVIEW_PAD` in, holding one inset passage row with
// a 2 pt bar.
pub const PREVIEW_W: f32 = 320.0;
pub const PREVIEW_PAD: f32 = 6.0;
pub const PREVIEW_ROW_PAD: f32 = 8.0;
pub const PREVIEW_ROW_RADIUS: f32 = 6.0;
pub const PREVIEW_BAR: f32 = 2.0;
pub const PREVIEW_GAP: f32 = 4.0;
pub const PREVIEW_TAG_RADIUS: f32 = 3.0;

// Between two superscript numbers of one span: a thin space.
pub const CITE_GAP: &str = "\u{2009}";

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The citation numbers' font: `family`'s superscript figures (OpenType
/// `sups`) at medium weight. The weight keeps the shaper from merging a
/// number into the regular word before it and dropping `sups`.
pub fn marker_font(family: impl Into<SharedString>) -> Font {
    Font {
        features: FontFeatures(Arc::new(vec![("sups".into(), 1)])),
        weight: FontWeight::MEDIUM,
        ..gpui::font(family)
    }
}

/// The text of one span's citations: their numbers, thin-spaced when there
/// are several ("1", "1 3"). Numbers are 1-based, as the reader sees them.
pub fn marker_text(numbers: &[usize]) -> String {
    numbers
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(CITE_GAP)
}

/// The heading's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct HeadingStyle {
    pub title: Hsla,
    pub count: Hsla,
}

/// The column heading: the name (cut with an ellipsis) and its count. The
/// host adds `Role::Heading` and an accessible name with the count in words.
pub fn heading(
    style: &HeadingStyle,
    title: impl Into<SharedString>,
    count: impl Into<SharedString>,
) -> Div {
    div()
        .h(ui_rems(HEADING_H))
        .px(ui_rems(HEADING_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(HEADING_PAD_X))
        .text_size(ui_rems(HEADING_TEXT))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(style.title)
                .child(title.into()),
        )
        .child(
            div()
                .flex_none()
                .text_color(style.count)
                .child(count.into()),
        )
}

/// A source card's resolved colours: lit (a citation of it pointed at, or
/// its card open) and hover states already blended by the host.
#[derive(Clone, Copy, Debug)]
pub struct CardStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub tile: Hsla,
    pub tile_text: Hsla,
    pub title: Hsla,
    pub meta: Hsla,
    pub chevron: Hsla,
}

/// What a source's leading tile holds.
#[derive(Clone, Debug)]
pub enum Tile {
    /// The number its citations carry in the answer.
    Number(SharedString),
    /// An icon path, for a source the provider listed without saying which
    /// part of the reply it supports.
    Icon(SharedString),
}

/// The card plate: r10 corners, a hairline edge and a quiet fill, clipping
/// its folding body. Children: `header(…)`, then the host's folding body.
pub fn card(style: &CardStyle) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(ui_rems(CARD_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
}

/// The two-line header: the tile, the title (medium) over the site line,
/// both cut with an ellipsis, and an optional chevron (`icon`, turned by
/// `turn` radians as the card folds open). The host adds the id,
/// `Role::Button`, the accessible name and expanded state, focus ring, hover
/// and the click that folds the card.
pub fn header(
    style: &CardStyle,
    tile: Tile,
    title: impl Into<SharedString>,
    meta: impl Into<SharedString>,
    chevron: Option<(SharedString, Radians)>,
) -> Div {
    let tile = div()
        .size(ui_rems(TILE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(ui_rems(TILE_RADIUS))
        .bg(style.tile)
        .text_size(ui_rems(TILE_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.tile_text)
        .map(|el| match tile {
            Tile::Number(number) => el.child(number),
            Tile::Icon(path) => el.child(
                svg()
                    .path(path)
                    .size(ui_rems(TILE_ICON))
                    .flex_none()
                    .text_color(style.tile_text),
            ),
        });
    let line = |size: f32, height: f32| {
        div()
            .w_full()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_size(ui_rems(size))
            .line_height(ui_rems(height))
    };
    div()
        .h(ui_rems(HEADER_H))
        .flex_none()
        .px(ui_rems(HEADER_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(HEADER_GAP))
        .child(tile)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(ui_rems(LINE_GAP))
                .child(
                    line(TITLE_TEXT, TITLE_LINE)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(style.title)
                        .child(title.into()),
                )
                .child(
                    line(META_TEXT, META_LINE)
                        .text_color(style.meta)
                        .child(meta.into()),
                ),
        )
        .when_some(chevron, |el, (path, turn)| {
            el.child(
                div()
                    .size(ui_rems(TILE))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(path)
                            .size(ui_rems(CHEVRON))
                            .flex_none()
                            .text_color(style.chevron)
                            .with_transformation(Transformation::rotate(turn)),
                    ),
            )
        })
}

/// The folded body's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct QuoteStyle {
    pub rule: Hsla,
    pub rail: Hsla,
    pub background: Hsla,
    pub text: Hsla,
}

/// The folded body: a rule, then `passage` (the host's text, its backing
/// words already marked) in the blockquote. The host appends its meta lane
/// (copy, open) after this.
pub fn quote(style: &QuoteStyle, passage: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex_none()
        .flex()
        .flex_col()
        .child(div().h(gpui::px(1.0)).flex_none().bg(style.rule))
        .child(
            div().px(ui_rems(QUOTE_INSET)).pt(ui_rems(QUOTE_INSET)).child(
                div()
                    .relative()
                    .border_l(ui_rems(QUOTE_RAIL))
                    .border_color(style.rail)
                    .bg(style.background)
                    .rounded_tr(ui_rems(QUOTE_RADIUS))
                    .rounded_br(ui_rems(QUOTE_RADIUS))
                    .pl(ui_rems(QUOTE_PAD_L))
                    .pr(ui_rems(QUOTE_PAD_R))
                    .py(ui_rems(QUOTE_PAD_Y))
                    .text_size(ui_rems(QUOTE_TEXT))
                    .line_height(ui_rems(QUOTE_LINE))
                    .text_color(style.text)
                    .child(passage),
            ),
        )
}

/// The citation preview's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct PreviewStyle {
    pub row: Hsla,
    pub bar: Hsla,
    pub site: Hsla,
    pub tag: Hsla,
    pub title: Hsla,
    pub snippet: Hsla,
}

/// What a pointed-at citation previews: one passage row holding the site in
/// `mono` with the citation's number as a tag, the page title and, when the
/// provider quoted one, the passage. The host shows it as the citation's
/// tooltip inside its own `PREVIEW_W` popover surface, `PREVIEW_PAD` in.
pub fn preview(
    style: &PreviewStyle,
    mono: impl Into<SharedString>,
    number: Option<SharedString>,
    site: impl Into<SharedString>,
    title: impl Into<SharedString>,
    snippet: Option<SharedString>,
) -> Div {
    let site_line = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(PREVIEW_PAD))
        .font_family(mono.into())
        .text_size(ui_rems(META_TEXT))
        .line_height(ui_rems(META_LINE))
        .text_color(style.site)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(site.into()),
        )
        .when_some(number, |el, number| {
            el.child(
                div()
                    .flex_none()
                    .px(ui_rems(PREVIEW_GAP))
                    .rounded(ui_rems(PREVIEW_TAG_RADIUS))
                    .bg(style.tag)
                    .child(number),
            )
        });
    let body = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(ui_rems(PREVIEW_GAP))
        .child(site_line)
        .child(
            div()
                .text_size(ui_rems(TITLE_TEXT))
                .line_height(ui_rems(TITLE_LINE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.title)
                .child(title.into()),
        )
        .when_some(snippet, |el, snippet| {
            el.child(
                div()
                    .min_w_0()
                    .text_size(ui_rems(TITLE_TEXT))
                    .line_height(ui_rems(TITLE_LINE))
                    .text_color(style.snippet)
                    .child(snippet),
            )
        });
    div()
        .w_full()
        .flex()
        .flex_row()
        .gap(ui_rems(PREVIEW_ROW_PAD))
        .p(ui_rems(PREVIEW_ROW_PAD))
        .rounded(ui_rems(PREVIEW_ROW_RADIUS))
        .bg(style.row)
        .child(
            div()
                .flex_none()
                .w(ui_rems(PREVIEW_BAR))
                .rounded(ui_rems(PREVIEW_BAR / 2.0))
                .bg(style.bar),
        )
        .child(body)
}
