// PINK's automation editor (automations.rs `render_editor`, with
// `input_frame`): the form that creates or edits a scheduled prompt, inside
// a settings section card. These are its own layout parts: a group (a card
// row holding a semibold heading over its controls), a captioned field and
// its frame, a setting (a label on a fixed column beside its control), the
// schedule in words beside a clock, the More options disclosure, the inline
// rows that pair a control with its unit, the select menu's captions and the
// review column of the footer. UI rethink decision 54: drawn here, consumed
// by PINK through a pinned revision.
//
// The rest of the editor is other packages the host already pins: the card
// and its rows (`pink-settings-section`), buttons and choice chips
// (`pink-button`), the segmented track (`pink-segmented-control`), the
// switch (`pink-switch`), the select's menu card (`pink-menu-card`) and the
// model chips (`pink-trigger-chip`).
//
// The package draws only. The host supplies the resolved colours, the text
// inputs, the copy and the controls; it adds the ids, roles and accessible
// names, focus and tab stops, and the actions. Plain `//` comments: the
// pink-ui gallery compiles this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems, svg};

// A group: its heading over its controls, in pt at the 16 px default.
pub const GROUP_GAP: f32 = 10.0;
pub const HEADING_TEXT: f32 = 12.0;
pub const HEADING_LINE_HEIGHT: f32 = 16.0;

// A field: its caption over its frame.
pub const CAPTION_TEXT: f32 = 11.0;
pub const CAPTION_LINE_HEIGHT: f32 = 14.0;
pub const FIELD_GAP: f32 = 4.0;
// The minimum widths fields wrap below: a name or other wide value, a date,
// and a short value (a time, minutes or a number).
pub const FIELD_MIN_WIDE: f32 = 160.0;
pub const FIELD_MIN_DATE: f32 = 120.0;
pub const FIELD_MIN_SHORT: f32 = 96.0;

// A field's frame around the host's text input. The corners stay fixed.
pub const FRAME_PAD_X: f32 = 10.0;
pub const FRAME_PAD_Y: f32 = 6.0;
pub const FRAME_RADIUS: f32 = 8.0;
pub const FRAME_TEXT: f32 = 13.0;

// A setting: its label on a fixed column, the control beside it.
pub const LABEL_WIDTH: f32 = 112.0;
pub const LABEL_TEXT: f32 = 12.0;
pub const LABEL_LINE_HEIGHT: f32 = 16.0;
pub const SETTING_GAP_X: f32 = 12.0;
pub const SETTING_GAP_Y: f32 = 6.0;

// A row of choice chips (schedule kinds, weekdays).
pub const CHOICE_GAP: f32 = 6.0;

// The schedule in words beside a clock.
pub const SUMMARY_GAP: f32 = 6.0;
pub const SUMMARY_ICON: f32 = 14.0;
pub const SUMMARY_TEXT: f32 = 13.0;
pub const SUMMARY_LINE_HEIGHT: f32 = 18.0;

// The More options disclosure.
pub const DISCLOSURE_GAP: f32 = 8.0;
pub const DISCLOSURE_ICON: f32 = 12.0;
pub const DISCLOSURE_RADIUS: f32 = 4.0;

// A control with its unit or description on one line (the archive switch,
// the lateness field), and the lateness field's width.
pub const INLINE_GAP: f32 = 8.0;
pub const LATENESS_WIDTH: f32 = 72.0;

// The select pop-ups: the trigger's width range, the menu's width, its
// list's height and an option's caption.
pub const SELECT_MIN_WIDTH: f32 = 180.0;
pub const SELECT_MAX_WIDTH: f32 = 320.0;
pub const MENU_WIDTH: f32 = 320.0;
pub const MENU_LIST_MAX_HEIGHT: f32 = 280.0;
pub const MENU_CAPTION_MAX_WIDTH: f32 = 200.0;
pub const MENU_CAPTION_TEXT: f32 = 11.0;

// The New chat / Existing chat segmented track.
pub const SEGMENTED_WIDTH: f32 = 220.0;

// The footer's review lines.
pub const REVIEW_GAP: f32 = 2.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The editor's resolved colours.
pub struct EditorStyle {
    /// Setting labels and the schedule in words.
    pub text: Hsla,
    /// Group headings and the disclosure's label.
    pub heading: Hsla,
    /// Field captions and the disclosure's chevron.
    pub caption: Hsla,
    /// The schedule's clock.
    pub accent: Hsla,
}

/// A field frame's resolved colours.
pub struct FrameStyle {
    pub border: Hsla,
    pub background: Hsla,
}

/// A group: the host's card row (`pink-settings-section` `card_row`, one
/// hairline above) as a column of its heading and controls.
pub fn group(row: Div, style: &EditorStyle, title: Option<SharedString>) -> Div {
    row.flex_col()
        .items_start()
        .gap(ui_rems(GROUP_GAP))
        .when_some(title, |group, title| {
            group.child(heading(style.heading, title))
        })
}

/// A group's heading, and the disclosure's label: 12/16 semibold.
pub fn heading(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(ui_rems(HEADING_TEXT))
        .line_height(ui_rems(HEADING_LINE_HEIGHT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(text.into())
}

/// A field's caption: 11/14.
pub fn caption(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .text_size(ui_rems(CAPTION_TEXT))
        .line_height(ui_rems(CAPTION_LINE_HEIGHT))
        .text_color(color)
        .child(text.into())
}

/// A field's frame: full width, padded, bordered and filled. The host adds
/// its text input.
pub fn frame(style: &FrameStyle) -> Div {
    div()
        .w_full()
        .px(ui_rems(FRAME_PAD_X))
        .py(ui_rems(FRAME_PAD_Y))
        .rounded(px(FRAME_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .text_size(ui_rems(FRAME_TEXT))
}

/// A labelled field: the caption over the host's frame. Fields share a line
/// and wrap below `min` (pt at the 16 px default).
pub fn field(
    style: &EditorStyle,
    label: impl Into<SharedString>,
    min: f32,
    frame: impl IntoElement,
) -> Div {
    div()
        .flex_1()
        .min_w(ui_rems(min))
        .flex()
        .flex_col()
        .gap(ui_rems(FIELD_GAP))
        .child(caption(style.caption, label))
        .child(frame)
}

/// A setting inside a group: its label on a fixed column, the control
/// beside it. Both wrap in a narrow pane.
pub fn setting(
    style: &EditorStyle,
    label: impl Into<SharedString>,
    control: impl IntoElement,
) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(ui_rems(SETTING_GAP_X))
        .gap_y(ui_rems(SETTING_GAP_Y))
        .child(
            div()
                .flex_none()
                .w(ui_rems(LABEL_WIDTH))
                .text_size(ui_rems(LABEL_TEXT))
                .line_height(ui_rems(LABEL_LINE_HEIGHT))
                .text_color(style.text)
                .child(label.into()),
        )
        .child(div().min_w_0().child(control))
}

/// The schedule in words beside a clock (`icon`, an SVG path the host
/// bundles): 13/18 medium. The host adds the id.
pub fn schedule_line(
    style: &EditorStyle,
    icon: impl Into<SharedString>,
    words: impl Into<SharedString>,
) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(SUMMARY_GAP))
        .child(
            svg()
                .path(icon.into())
                .size(ui_rems(SUMMARY_ICON))
                .flex_none()
                .text_color(style.accent),
        )
        .child(
            div()
                .min_w_0()
                .text_size(ui_rems(SUMMARY_TEXT))
                .line_height(ui_rems(SUMMARY_LINE_HEIGHT))
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.text)
                .child(words.into()),
        )
}

/// The More options disclosure: its label in the group heading's style and
/// the host's chevron (up when open, down when folded, `DISCLOSURE_ICON`
/// square in the caption colour). The corners round its focus ring. The
/// host adds the id, the button role, the expanded state, the tab stop, the
/// focus ring and the toggle.
pub fn disclosure(
    style: &EditorStyle,
    label: impl Into<SharedString>,
    chevron: impl IntoElement,
) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(DISCLOSURE_GAP))
        .rounded(px(DISCLOSURE_RADIUS))
        .cursor_pointer()
        .child(heading(style.heading, label))
        .child(chevron)
}

/// A control and its unit or description on one line.
pub fn inline() -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(INLINE_GAP))
}

/// The lateness field's fixed width around the host's frame.
pub fn lateness(frame: impl IntoElement) -> Div {
    div().w(ui_rems(LATENESS_WIDTH)).child(frame)
}

/// A select menu option's caption after its title: 11, truncated at
/// `MENU_CAPTION_MAX_WIDTH`.
pub fn menu_caption(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .max_w(ui_rems(MENU_CAPTION_MAX_WIDTH))
        .truncate()
        .text_size(ui_rems(MENU_CAPTION_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The footer's review column beside its buttons: the host's lines, which
/// hold `text_min` (pt at the 16 px default) before the buttons wrap under.
pub fn review(text_min: f32) -> Div {
    div()
        .flex_1()
        .min_w(ui_rems(text_min))
        .flex()
        .flex_col()
        .gap(ui_rems(REVIEW_GAP))
}
