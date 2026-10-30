// PINK's Autonomy Dial (issue #372): an agent's actions sorted into the
// groups its permission level puts them in, "Unattended" and "Needs
// approval". The level itself is PINK's access chip and Permission mode
// menu; this package draws what sits under it: each group's heading with its
// count, one row per action (its glyph, its name and who decides it) and the
// quiet line an empty group holds.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours, the icon
// paths and every word; it adds the ids, roles and accessible names, focus,
// hover fills, tooltips and any motion. Plain `//` comments: the pink-ui
// gallery compiles this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, rems, svg};

// A group heading (PINK's Activity section heading): `HEAD_H` tall, the
// name and count at `HEAD_TEXT`, and `GROUP_GAP` above the next group.
pub const HEAD_H: f32 = 28.0;
pub const HEAD_TEXT: f32 = 11.0;
pub const GROUP_GAP: f32 = 12.0;
// One action row (PINK's compact sidebar row): `ROW_H` tall with `PAD_X` at
// both ends, a `MARK_SLOT` glyph slot then `MARK_GAP`, the `LABEL_TEXT`
// name and the `META_TEXT` word saying who decides.
pub const ROW_H: f32 = 32.0;
pub const PAD_X: f32 = 8.0;
pub const MARK_SLOT: f32 = 16.0;
pub const MARK_GAP: f32 = 8.0;
pub const LABEL_TEXT: f32 = 13.0;
pub const META_TEXT: f32 = 11.0;
// The line an empty group holds.
pub const EMPTY_TEXT: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// A heading's resolved colours: the name, then the quieter count.
#[derive(Clone, Copy, Debug)]
pub struct HeadingStyle {
    pub title: Hsla,
    pub count: Hsla,
}

/// A row's resolved colours: the glyph, the action's name and the meta word
/// (quiet, or the warning tone for something that runs with no one asked).
#[derive(Clone, Copy, Debug)]
pub struct RowStyle {
    pub icon: Hsla,
    pub label: Hsla,
    pub meta: Hsla,
}

/// The column the groups stack in, `GROUP_GAP` apart.
pub fn groups() -> Div {
    div().w_full().flex().flex_col().gap(ui_rems(GROUP_GAP))
}

/// One group: its heading over its rows (or its empty line).
pub fn group() -> Div {
    div().w_full().flex().flex_col()
}

/// A group heading: the name (semibold, cut with an ellipsis) and its count
/// at the end. The host adds `Role::Heading` and an accessible name that
/// says the count in words.
pub fn heading(style: &HeadingStyle, title: impl Into<SharedString>, count: usize) -> Div {
    div()
        .h(ui_rems(HEAD_H))
        .w_full()
        .px(ui_rems(PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(MARK_GAP))
        .text_size(ui_rems(HEAD_TEXT))
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
                .child(SharedString::from(count.to_string())),
        )
}

/// One action: the glyph in its slot (left empty without one, so names line
/// up), the name taking the room and cut with an ellipsis, and the meta word
/// at the end, which never gives way. The host can append a trailing control
/// (a lock) as a child.
pub fn row(
    style: &RowStyle,
    icon: Option<SharedString>,
    label: impl Into<SharedString>,
    meta: impl Into<SharedString>,
) -> Div {
    div()
        .h(ui_rems(ROW_H))
        .w_full()
        .px(ui_rems(PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(MARK_GAP))
        .child(
            div()
                .flex_none()
                .size(ui_rems(MARK_SLOT))
                .flex()
                .items_center()
                .justify_center()
                .when_some(icon, |slot, icon| {
                    slot.child(
                        svg()
                            .path(icon)
                            .flex_none()
                            .size(ui_rems(MARK_SLOT))
                            .text_color(style.icon),
                    )
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(ui_rems(LABEL_TEXT))
                .text_color(style.label)
                .child(label.into()),
        )
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_size(ui_rems(META_TEXT))
                .text_color(style.meta)
                .child(meta.into()),
        )
}

/// The quiet line a group holds when no action is in it ("Nothing needs
/// approval"), one row tall.
pub fn empty(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .h(ui_rems(ROW_H))
        .w_full()
        .px(ui_rems(PAD_X))
        .flex()
        .items_center()
        .text_size(ui_rems(EMPTY_TEXT))
        .text_color(color)
        .child(text.into())
}
