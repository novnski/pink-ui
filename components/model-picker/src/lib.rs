// PINK's model menu (pickers.rs `render_harness_model_popover` and
// `render_model_row`): the card the composer's model chip opens. Across the
// top a strip of 32 pt tabs (the Favorites star, the Recent clock and one
// mark per provider) with a 2 pt accent bar under the viewed one, sitting
// on the strip's hairline. Under it a search row, then a fixed band of
// compact model rows on a whisper of wash, each with its name, an optional
// "New" capsule and attribution, a ⌘1–⌘9 key cap and a star. A favourites
// view lists two-line rows with the provider's mark and name underneath.
// The tray pinned below holds the Reasoning and option rows, each naming
// its choice with a chevron.
//
// Every design size is in pt at the 16 px default and becomes rems
// (`ui_rems`), so the whole card grows with the host's Interface size. The
// tab bar's offset is the strip's own inset, so it stays on the hairline at
// every size.
//
// The package draws only. The host supplies the glyph elements (its own
// assets), the text and the colours, already resolved and blended over its
// hover fade, and adds what crates.io GPUI 0.2.2 can't express (roles,
// accessible names, focus and its ring), the hover washes and the
// behaviour (press, click, keyboard, scrolling).

use gpui::{
    AnyElement, BoxShadow, Div, ElementId, FontWeight, Hsla, Length, Rems, SharedString, Stateful,
    div, prelude::*, px, rems,
};

/// A design size (pt at the 16 px default) as rems, the same formula as the
/// host's `ui_rems`.
pub fn ui_rems(design: f32) -> Rems {
    rems(design / 16.0)
}

/// The card's width.
pub const CARD_W: f32 = 320.0;
/// The card's inset around its bands (PINK's `CARD_INSET`).
pub const CARD_INSET: f32 = 4.0;
/// A row's corner (PINK's `MENU_ITEM_RADIUS`).
pub const ITEM_RADIUS: f32 = 8.0;
/// The gap between stacked rows.
pub const ROW_SPACING: f32 = 2.0;

/// The tab strip's and the search row's height.
pub const STRIP_H: f32 = 40.0;
/// A tab's side.
pub const TAB: f32 = 32.0;
/// A tab's corner.
pub const TAB_RADIUS: f32 = 8.0;
/// The gap between tabs.
pub const TAB_GAP: f32 = 2.0;
/// A provider's mark on its tab.
pub const PROVIDER_MARK: f32 = 16.0;
/// The Favorites star and the Recent clock.
pub const TAB_GLYPH: f32 = 15.0;
/// The viewed tab's bar: its height and its inset from the tab's sides.
pub const BAR_H: f32 = 2.0;
pub const BAR_INSET: f32 = 6.0;

/// The search row's horizontal padding, gap, glyph and type.
pub const SEARCH_PAD_X: f32 = 10.0;
pub const SEARCH_GAP: f32 = 8.0;
pub const SEARCH_GLYPH: f32 = 14.0;
pub const SEARCH_TEXT: f32 = 13.0;

/// The model band's height, roughly seven compact rows.
pub const LIST_H: f32 = 216.0;
/// A model row's padding and the gap between its parts.
pub const ROW_PAD_X: f32 = 8.0;
pub const ROW_PAD_Y: f32 = 6.0;
pub const ROW_PAD_Y_COMPACT: f32 = 5.0;
pub const ROW_GAP: f32 = 10.0;
/// The gap between a row's name, badge and attribution.
pub const NAME_GAP: f32 = 6.0;
/// The model name's and the meta line's type, and the meta line's mark.
pub const NAME_TEXT: f32 = 12.5;
pub const META_TEXT: f32 = 11.0;
pub const META_MARK: f32 = 11.0;
/// The star button's side and glyph.
pub const STAR: f32 = 22.0;
pub const STAR_GLYPH: f32 = 13.0;
/// The "New" capsule.
pub const BADGE_H: f32 = 16.0;
pub const BADGE_PAD_X: f32 = 5.0;
pub const BADGE_RADIUS: f32 = 4.0;
pub const BADGE_TEXT: f32 = 10.5;
pub const BADGE_LINE: f32 = 14.0;

/// The tray: its tallest before it scrolls, a row's height, the widest a
/// value grows and the chevron.
pub const TRAY_MAX_H: f32 = 236.0;
pub const TRAY_ROW_H: f32 = 30.0;
pub const VALUE_MAX_W: f32 = 100.0;
pub const CHEVRON: f32 = 12.0;
/// A tray row's nested choices card, and its check.
pub const CHOICES_W: f32 = 232.0;
pub const CHOICES_MAX_H: f32 = 240.0;
pub const CHOICE_CHECK: f32 = 14.0;

/// The tab strip across the top of the card, over a full-bleed hairline.
pub fn tab_strip(id: impl Into<ElementId>, hairline: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .h(ui_rems(STRIP_H))
        .px(px(CARD_INSET))
        .border_b_1()
        .border_color(hairline)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(TAB_GAP))
}

/// One tab: a `TAB` square holding the host's glyph, with the accent bar
/// when it is the viewed one.
pub fn tab(id: impl Into<ElementId>, glyph: AnyElement, viewed: bool, bar: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .w(ui_rems(TAB))
        .h(ui_rems(TAB))
        .rounded(px(TAB_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(glyph)
        .when(viewed, |el| el.child(tab_bar(bar)))
}

/// The viewed tab's bar: a 2 pt capsule on the strip's bottom hairline. The
/// tab sits centred in the strip, so the bar drops by the strip's inset.
pub fn tab_bar(tint: Hsla) -> Div {
    div()
        .absolute()
        .bottom(ui_rems(-(STRIP_H - TAB) / 2.0))
        .left(px(BAR_INSET))
        .right(px(BAR_INSET))
        .h(px(BAR_H))
        .rounded(px(BAR_H / 2.0))
        .bg(tint)
}

/// The search row: the host's magnifier and its borderless field over a
/// full-bleed hairline (a strip, not a well).
pub fn search_row(glyph: AnyElement, field: AnyElement, hairline: Hsla) -> Div {
    div()
        .flex_none()
        .h(ui_rems(STRIP_H))
        .px(ui_rems(SEARCH_PAD_X))
        .border_b_1()
        .border_color(hairline)
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(SEARCH_GAP))
        .child(glyph)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(ui_rems(SEARCH_TEXT))
                .child(field),
        )
}

/// The model band: `height` tall (the host sizes it, `LIST_H` at most) on a
/// whisper of wash between the pinned chrome above and the tray below.
pub fn list_band(id: impl Into<ElementId>, height: impl Into<Length>, wash: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex_none()
        .h(height.into())
        .py(px(CARD_INSET))
        .bg(wash)
}

/// The provider line under a two-line row's name (the Favorites and Recent
/// views mix providers).
pub struct ProviderLine {
    /// The provider's mark, sized `META_MARK`.
    pub mark: AnyElement,
    pub name: SharedString,
}

/// What one model row shows.
pub struct ModelRow {
    pub name: SharedString,
    /// The "New" capsule (`new_badge`) after the name.
    pub badge: Option<AnyElement>,
    /// Which upstream provider serves the model, when several serve one
    /// name. Inline after the name on a compact row; after the provider on
    /// a two-line row, where the host words it ("via Anthropic").
    pub attribution: Option<SharedString>,
    /// `Some` draws the two-line row with the provider underneath.
    pub provider: Option<ProviderLine>,
    /// The host's ⌘1–⌘9 key cap.
    pub shortcut: Option<AnyElement>,
    /// The host's star button (`star_button`).
    pub star: AnyElement,
}

/// The colours one model row draws, already resolved by the host.
pub struct ModelRowStyle {
    pub text: Hsla,
    pub muted: Hsla,
    /// The two-line row's attribution.
    pub faint: Hsla,
    /// The selected wash or the keyboard's highlight; `None` at rest.
    pub fill: Option<Hsla>,
    /// The selected row's ring.
    pub shadows: Vec<BoxShadow>,
}

/// One model row: the body (name, badge, attribution, and the provider line
/// on a two-line row), the key cap and the star.
pub fn model_row(id: impl Into<ElementId>, row: ModelRow, style: &ModelRowStyle) -> Stateful<Div> {
    let compact = row.provider.is_none();
    let name = div()
        .text_size(ui_rems(NAME_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
        .child(row.name);
    let body = match row.provider {
        None => div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(ui_rems(NAME_GAP))
            .child(name.flex_none().max_w_full().truncate())
            .children(row.badge)
            .when_some(row.attribution, |el, attribution| {
                el.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(ui_rems(META_TEXT))
                        .text_color(style.muted)
                        .child(attribution),
                )
            }),
        Some(provider) => div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(ROW_SPACING))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(ui_rems(NAME_GAP))
                    .child(name.min_w_0().truncate())
                    .children(row.badge),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(ui_rems(NAME_GAP))
                    .child(provider.mark)
                    .child(
                        div()
                            .flex_none()
                            .text_size(ui_rems(META_TEXT))
                            .text_color(style.muted)
                            .child(provider.name),
                    )
                    .when_some(row.attribution, |el, attribution| {
                        el.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(ui_rems(META_TEXT))
                                .text_color(style.faint)
                                .child(attribution),
                        )
                    }),
            ),
    };
    let mut el = div()
        .id(id)
        .px(ui_rems(ROW_PAD_X))
        .py(px(if compact {
            ROW_PAD_Y_COMPACT
        } else {
            ROW_PAD_Y
        }))
        .rounded(px(ITEM_RADIUS))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .cursor_pointer();
    if let Some(fill) = style.fill {
        el = el.bg(fill);
    }
    if !style.shadows.is_empty() {
        el = el.shadow(style.shadows.clone());
    }
    el.child(body).children(row.shortcut).child(row.star)
}

/// The slot one model row sits in: the gap under it.
pub fn row_slot() -> Div {
    div().pb(px(ROW_SPACING))
}

/// A row's star: a `STAR` square around the host's glyph.
pub fn star_button(id: impl Into<ElementId>, glyph: AnyElement) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(ui_rems(STAR))
        .h(ui_rems(STAR))
        .rounded(px(ITEM_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(glyph)
}

/// The "New" capsule beside a model's name: `tone` on a `fill` of the
/// accent.
pub fn new_badge(label: impl Into<SharedString>, tone: Hsla, fill: Hsla) -> Div {
    div()
        .flex_none()
        .h(ui_rems(BADGE_H))
        .px(ui_rems(BADGE_PAD_X))
        .flex()
        .items_center()
        .rounded(px(BADGE_RADIUS))
        .bg(fill)
        .text_size(ui_rems(BADGE_TEXT))
        .line_height(ui_rems(BADGE_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(tone)
        .child(label.into())
}

/// The "Default" marker beside a choice: bare text, no plate.
pub fn default_badge(label: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(META_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label.into())
}

/// The tray pinned under the list, over a full-bleed hairline; a long stack
/// scrolls inside it rather than growing the card.
pub fn tray(id: impl Into<ElementId>, hairline: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .border_t_1()
        .border_color(hairline)
        .max_h(ui_rems(TRAY_MAX_H))
        .overflow_y_scroll()
        .px(px(CARD_INSET))
}

/// The tray's stack of rows.
pub fn tray_rows() -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(ROW_SPACING))
        .py(px(CARD_INSET))
}

/// A tray row's label, value and chevron, for the host's menu row (which
/// is `TRAY_ROW_H` tall).
pub fn setting_parts(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    chevron: AnyElement,
    muted: Hsla,
) -> [AnyElement; 3] {
    [
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .child(label.into())
            .into_any_element(),
        div()
            .max_w(ui_rems(VALUE_MAX_W))
            .truncate()
            .text_color(muted)
            .child(value.into())
            .into_any_element(),
        chevron,
    ]
}
