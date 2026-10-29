// PINK's Changes pane chrome around the diff rows (changes.rs
// `render_file_header`, `render_sticky_file_header`, `render_header_strip`,
// `render_header_controls`, `render_ref_selector`, `render_scope_menu` and
// `render_ref_menu`): a file's header (the turning disclosure, the file-type
// artwork, the mono path, BIN and the `+N` / `-N` counts) as a list row or as
// the sticky copy held over the list's top edge; the summary strip over the
// list; and the pane header's controls: the scope trigger, the History
// surface's branch title, the `{branch} -> {base}` ref selector, the Git chip,
// the commit-pinned pane's identity chip and subject, and the content of the
// scope and base menus' rows. The menus' cards and rows are `pink-menu-card`
// and the header's icon toggles `pink-icon-button`, so this package adds no
// second card, row or button. UI rethink decision 54: drawn here, consumed
// by PINK through a pinned revision.
//
// The package draws only. The host supplies the glyphs and artwork (its own
// icon assets), every colour already resolved (each fill already blended over
// its hover fade, a sticky header's opaque plate), the mono family and the
// text; it adds the ids' listeners, roles, accessible names, focus, tooltips,
// occlusion, folding and its tween, the sticky header's push offset and
// frost, the menus and their placement, and the diff itself. Plain `//`
// comments: the pink-ui gallery compiles this file in place with `include!`
// for its story.

use gpui::{
    AbsoluteLength, Div, ElementId, Hsla, Img, IntoElement, SharedString, Stateful, Svg,
    Transformation, div, prelude::*, px, radians, rems,
};

/// A file header's height, in fixed pixels (PINK `FILE_HEADER_HEIGHT`, the
/// titlebar height): the virtualized list measures headers by it.
pub const FILE_HEADER_HEIGHT: f32 = 38.0;
/// A file header's side inset, in fixed pixels (PINK `Theme::SPACE_MD`).
pub const FILE_HEADER_PAD_X: f32 = 12.0;
/// The gap between a header's parts, in pt at the 16 px default.
pub const FILE_HEADER_GAP: f32 = 8.0;
/// The disclosure's slot and its glyph, in pt at the 16 px default.
pub const DISCLOSURE_SLOT: f32 = 14.0;
pub const DISCLOSURE_GLYPH: f32 = 12.0;
/// The file-type artwork, in pt at the 16 px default.
pub const ARTWORK: f32 = 14.0;
/// The path's and the counts' type, in pt at the 16 px default.
pub const PATH_TEXT: f32 = 12.0;
pub const COUNT_TEXT: f32 = 11.0;

/// The summary strip's height and side inset, in fixed pixels (PINK
/// `surface_chrome::HEADER_HEIGHT`, `Theme::SPACE_LG`), and its gap and
/// label type in pt at the 16 px default.
pub const STRIP_HEIGHT: f32 = 38.0;
pub const STRIP_PAD_X: f32 = 16.0;
pub const STRIP_GAP: f32 = 10.0;
pub const STRIP_TEXT: f32 = 12.0;

/// The pane header's controls: their height and corner and the gap between
/// them, in fixed pixels (PINK `surface_chrome::CONTROL_SIZE`,
/// `CONTROL_RADIUS`, `CONTROL_GAP`).
pub const CONTROL_HEIGHT: f32 = 28.0;
pub const CONTROL_RADIUS: f32 = 6.0;
pub const CONTROL_GAP: f32 = 4.0;
/// The History title's widest, in pt at the 16 px default.
pub const HISTORY_TITLE_MAX_WIDTH: f32 = 160.0;
/// The scope and base menus' widths, in pt at the 16 px default.
pub const SCOPE_MENU_WIDTH: f32 = 180.0;
pub const REF_MENU_WIDTH: f32 = 240.0;
/// The base menu's list height cap, in pt at the 16 px default, and the
/// gap between menu rows, in fixed pixels.
pub const REF_LIST_MAX_HEIGHT: f32 = 240.0;
pub const MENU_ROW_GAP: f32 = 2.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

// A flex factor through the style, so the file compiles on crates.io GPUI
// 0.2.2 (whose `flex_shrink` takes no factor).
fn shrink<E: Styled>(mut el: E, factor: f32) -> E {
    el.style().flex_shrink = Some(factor);
    el
}

// ---------------------------------------------------------------------------
// File headers
// ---------------------------------------------------------------------------

/// Where a file header sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileHeaderPlacement {
    /// In the list. Every header after the first carries a rule on top.
    Row { first: bool },
    /// Held over the list's top edge, a rule underneath.
    Sticky,
}

/// What a file header draws, resolved by the host.
pub struct FileHeaderStyle {
    /// The plate, already blended over the hover fade (opaque for the
    /// sticky copy, which floats over the rows).
    pub fill: Hsla,
    /// The rule above a row (PINK `hairline(0.04)`) or under the sticky copy
    /// (PINK `border`).
    pub rule: Hsla,
}

/// A file header: `FILE_HEADER_HEIGHT` tall and full width, its parts
/// `FILE_HEADER_GAP` apart inside `FILE_HEADER_PAD_X`. The host adds the
/// disclosure, artwork, path and counts, the hover listener and the fold.
pub fn file_header(
    id: impl Into<ElementId>,
    placement: FileHeaderPlacement,
    style: &FileHeaderStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .h(px(FILE_HEADER_HEIGHT))
        .when(
            placement == FileHeaderPlacement::Row { first: false },
            |el| el.border_t_1().border_color(style.rule),
        )
        .when(placement == FileHeaderPlacement::Sticky, |el| {
            el.border_b_1().border_color(style.rule)
        })
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(FILE_HEADER_GAP))
        .px(px(FILE_HEADER_PAD_X))
        .cursor_pointer()
        .bg(style.fill)
}

/// The disclosure: one glyph turned a quarter as the file opens (`open`
/// runs from 0, folded, to 1, open), centred in its slot.
pub fn disclosure(glyph: Svg, open: f32, color: Hsla) -> Div {
    div()
        .flex_none()
        .size(ui_rems(DISCLOSURE_SLOT))
        .flex()
        .items_center()
        .justify_center()
        .child(
            glyph
                .with_transformation(Transformation::rotate(radians(
                    std::f32::consts::FRAC_PI_2 * open,
                )))
                .size(ui_rems(DISCLOSURE_GLYPH))
                .text_color(color),
        )
}

/// The file-type artwork.
pub fn artwork(artwork: Img) -> Img {
    artwork.size(ui_rems(ARTWORK)).flex_none()
}

/// The path, in the mono family, taking the header's free width and
/// truncating.
pub fn path(text: impl Into<SharedString>, mono: SharedString, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .font_family(mono)
        .text_size(ui_rems(PATH_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The tag a binary file carries in place of its line counts.
pub fn binary_tag(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(COUNT_TEXT))
        .text_color(color)
        .child(text.into())
}

/// One count (`+N` or `-N`) at the header's trailing edge.
pub fn count(text: impl Into<SharedString>, mono: SharedString, color: Hsla) -> Div {
    div()
        .flex_none()
        .font_family(mono)
        .text_size(ui_rems(COUNT_TEXT))
        .text_color(color)
        .child(text.into())
}

// ---------------------------------------------------------------------------
// Summary strip
// ---------------------------------------------------------------------------

/// The summary strip over the list: `STRIP_HEIGHT` tall, `STRIP_GAP` between
/// its parts inside `STRIP_PAD_X`, a `rule` underneath.
pub fn summary_strip(rule: Hsla) -> Div {
    div()
        .flex_none()
        .h(px(STRIP_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(STRIP_GAP))
        .px(px(STRIP_PAD_X))
        .border_b_1()
        .border_color(rule)
}

/// The strip's label ("3 Uncommitted changes"), truncating first.
pub fn summary_label(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(STRIP_TEXT))
        .text_color(color)
        .child(text.into())
}

/// One of the strip's totals (`+N` or `-N`).
pub fn summary_count(text: impl Into<SharedString>, mono: SharedString, color: Hsla) -> Div {
    div()
        .font_family(mono)
        .text_size(ui_rems(COUNT_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The badge a partial snapshot carries at the strip's trailing edge.
pub fn partial_badge(text: impl Into<SharedString>, fill: Hsla, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(COUNT_TEXT))
        .px(ui_rems(6.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(fill)
        .text_color(color)
        .child(text.into())
}

/// Free room that pushes what follows to the trailing edge.
pub fn spacer() -> Div {
    div().flex_1()
}

// ---------------------------------------------------------------------------
// Pane header
// ---------------------------------------------------------------------------

/// The pane header's row: its controls centred `CONTROL_GAP` apart.
pub fn header_row() -> Div {
    div()
        .size_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(CONTROL_GAP))
}

/// What the scope trigger draws, resolved by the host.
pub struct ScopeTriggerStyle {
    /// The plate, already blended over the hover fade (PINK `wash(0.05)`
    /// easing to `wash(0.14)`).
    pub fill: Hsla,
    pub text: Hsla,
    pub chevron: Hsla,
}

/// The scope dropdown's trigger: the scope's name and a chevron on a
/// `CONTROL_HEIGHT` plate. The host adds the menu and the listeners.
pub fn scope_trigger(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    chevron: Svg,
    style: &ScopeTriggerStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(CONTROL_HEIGHT))
        .px(ui_rems(8.0))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(6.0))
        .rounded(px(CONTROL_RADIUS))
        .cursor_pointer()
        .bg(style.fill)
        .child(
            div()
                .text_size(ui_rems(12.0))
                .line_height(ui_rems(14.0))
                .text_color(style.text)
                .child(label.into()),
        )
        .child(chevron.size(ui_rems(12.0)).text_color(style.chevron))
}

/// The History surface's title slot: the current branch in the mono family,
/// shrinking and truncating up to `HISTORY_TITLE_MAX_WIDTH`.
pub fn history_title(
    id: impl Into<ElementId>,
    branch: impl Into<SharedString>,
    mono: SharedString,
    color: Hsla,
) -> Stateful<Div> {
    shrink(
        div()
            .id(id)
            .min_w_0()
            .max_w(ui_rems(HISTORY_TITLE_MAX_WIDTH))
            .truncate()
            .h(px(CONTROL_HEIGHT))
            .px(ui_rems(8.0)),
        1.0,
    )
    .flex()
    .items_center()
    .font_family(mono)
    .text_size(ui_rems(11.5))
    .line_height(ui_rems(14.0))
    .text_color(color)
    .child(branch.into())
}

/// The History surface's commit count, taking the free width between the
/// title and the trailing controls.
pub fn history_count(count: impl IntoElement) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .child(count)
}

/// The header's trailing controls, at their natural width.
pub fn trailing() -> Div {
    div().flex_none().flex().items_center().gap(px(CONTROL_GAP))
}

/// The History surface's trailing controls, which may shrink.
pub fn history_trailing() -> Div {
    shrink(div().min_w_0(), 1.0)
        .flex()
        .items_center()
        .gap(px(CONTROL_GAP))
}

/// The Git chip that opens the Git card over the working tree.
pub fn git_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    fill: Hsla,
    color: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(CONTROL_HEIGHT))
        .px(ui_rems(8.0))
        .rounded(px(CONTROL_RADIUS))
        .flex()
        .items_center()
        .cursor_pointer()
        .bg(fill)
        .text_size(ui_rems(11.0))
        .text_color(color)
        .child(label.into())
}

/// A commit-pinned pane's identity: the short sha on a `CONTROL_HEIGHT`
/// plate.
pub fn commit_sha(
    text: impl Into<SharedString>,
    mono: SharedString,
    fill: Hsla,
    color: Hsla,
) -> Div {
    div()
        .flex_none()
        .h(px(CONTROL_HEIGHT))
        .px(ui_rems(6.0))
        .rounded(px(CONTROL_RADIUS))
        .flex()
        .items_center()
        .bg(fill)
        .font_family(mono)
        .text_size(ui_rems(11.0))
        .text_color(color)
        .child(text.into())
}

/// The pinned commit's subject, taking the free width and truncating.
pub fn commit_subject(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(12.0))
        .text_color(color)
        .child(text.into())
}

/// How strongly a ref name gives way when the header runs out of room: its
/// length squared (mono type, so characters track width), so the longer of
/// the branch and the base takes most of the loss and a short name keeps
/// its characters.
pub fn ref_shrink(name: &str) -> f32 {
    (name.chars().count().max(1) as f32).powi(2)
}

/// What the ref selector draws, resolved by the host.
pub struct RefSelectorStyle {
    pub mono: SharedString,
    /// The current branch (PINK `text_dim`).
    pub branch: Hsla,
    /// The arrow between the names (PINK `text_faint`).
    pub arrow: Hsla,
}

/// `{branch} -> {base}`: the current branch, the arrow and the base's
/// trigger (`ref_trigger`), set off from the scope trigger.
pub fn ref_selector(
    branch: impl Into<SharedString>,
    arrow: Svg,
    trigger: impl IntoElement,
    style: &RefSelectorStyle,
) -> Div {
    let branch: SharedString = branch.into();
    let weight = ref_shrink(&branch);
    div()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(6.0))
        .ml(px(CONTROL_GAP))
        .child(
            shrink(div().min_w_0(), weight)
                .truncate()
                .font_family(style.mono.clone())
                .text_size(ui_rems(11.5))
                .text_color(style.branch)
                .child(branch),
        )
        .child(
            arrow
                .size(ui_rems(12.0))
                .flex_none()
                .text_color(style.arrow),
        )
        .child(trigger)
}

/// What the base's trigger draws, resolved by the host.
pub struct RefTriggerStyle {
    pub mono: SharedString,
    /// The plate, already blended over the hover fade (PINK `wash(0.0)`
    /// easing to `wash(0.12)`).
    pub fill: Hsla,
    pub text: Hsla,
    pub chevron: Hsla,
}

/// The base's trigger: its name and a chevron, shrinking (by `ref_shrink`)
/// and truncating rather than pushing the header's buttons away. The host
/// adds the menu and the listeners.
pub fn ref_trigger(
    id: impl Into<ElementId>,
    base: impl Into<SharedString>,
    chevron: Svg,
    style: &RefTriggerStyle,
) -> Stateful<Div> {
    let base: SharedString = base.into();
    let weight = ref_shrink(&base);
    shrink(
        div()
            .id(id)
            .h(px(CONTROL_HEIGHT))
            .px(ui_rems(6.0))
            .min_w_0(),
        weight,
    )
    .flex()
    .flex_row()
    .items_center()
    .gap(ui_rems(4.0))
    .rounded(px(CONTROL_RADIUS))
    .cursor_pointer()
    .bg(style.fill)
    .child(
        div()
            .min_w_0()
            .truncate()
            .font_family(style.mono.clone())
            .text_size(ui_rems(11.5))
            .text_color(style.text)
            .child(base),
    )
    .child(
        chevron
            .size(ui_rems(11.0))
            .flex_none()
            .text_color(style.chevron),
    )
}

// ---------------------------------------------------------------------------
// Scope and base menus
// ---------------------------------------------------------------------------

/// The scope menu's rows, `MENU_ROW_GAP` apart.
pub fn scope_list() -> Div {
    div().flex().flex_col().gap(px(MENU_ROW_GAP))
}

/// A scope row's name, filling the row.
pub fn scope_row_label(text: impl Into<SharedString>) -> Div {
    div().flex_1().child(text.into())
}

/// The base menu's list of branches, `MENU_ROW_GAP` apart and capped at
/// `REF_LIST_MAX_HEIGHT`. The host adds the scrolling and its handle.
pub fn ref_list(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(MENU_ROW_GAP))
        .max_h(ui_rems(REF_LIST_MAX_HEIGHT))
}

/// A branch row's name in the mono family, truncating.
pub fn ref_row_label(text: impl Into<SharedString>, mono: SharedString) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .font_family(mono)
        .text_size(ui_rems(12.0))
        .child(text.into())
}

/// The base menu's line when no branch is listed or matches.
pub fn ref_list_empty(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .px(ui_rems(8.0))
        .py(ui_rems(6.0))
        .text_size(ui_rems(12.0))
        .text_color(color)
        .child(text.into())
}
