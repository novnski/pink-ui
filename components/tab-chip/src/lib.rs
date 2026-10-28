// PINK's titlebar tab (workbench/render.rs `render_workbench_tab`): one chat
// tab in a group's strip, or a lone chat's title in the same slot, with the
// parts the tab carries (the Main badge, the close slot, the unread dot and
// the title's menu chevron). UI rethink decision 54: drawn here, consumed by
// PINK through a pinned revision.
//
// The package draws only. The host supplies the text colour and the
// selected fill already resolved for the strip's focus, the glyphs (its own
// icon assets), the spinner and the title; it adds the id, the hover wash,
// the role, accessible name and focus ring, the tooltip, the listeners, the
// drag and the measurement. Plain `//` comments: the pink-ui gallery compiles
// this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// A tab's height. Fixed: the titlebar strip does not grow with the
/// Interface size.
pub const TAB_HEIGHT: f32 = 28.0;
/// A tab's narrowest width in a roomy strip and in a compact one.
pub const TAB_MIN_WIDTH: f32 = 72.0;
pub const TAB_MIN_WIDTH_COMPACT: f32 = 40.0;
/// A tab's widest width.
pub const TAB_MAX_WIDTH: f32 = 220.0;
/// A tab's corner radius.
pub const TAB_RADIUS: f32 = 8.0;
/// The gap between a tab's parts.
pub const PART_GAP: f32 = 6.0;
/// The leading padding, and the trailing one without a close slot.
pub const PAD_LEFT: f32 = 8.0;
pub const PAD_RIGHT: f32 = 8.0;
/// The trailing padding beside a close slot: the control's own inset
/// carries the rest.
pub const PAD_RIGHT_CLOSABLE: f32 = 4.0;
/// A tab's label, in pt at the 16 px default (one medium weight for every
/// tab, so activation never reflows the strip).
pub const TAB_TEXT: f32 = 12.5;
/// A title's label: chrome type, in pt at the 16 px default.
pub const TITLE_TEXT: f32 = 13.0;
pub const TITLE_LINE_HEIGHT: f32 = 18.0;
/// The agent's mark, the role glyph and a status glyph.
pub const HARNESS_MARK: f32 = 14.0;
pub const ROLE_GLYPH: f32 = 11.0;
pub const STATUS_GLYPH: f32 = 11.0;
/// The unread dot's diameter.
pub const UNREAD_DOT: f32 = 6.0;
/// The close slot: the small control step, its radius and glyph.
pub const CLOSE_SIZE: f32 = 20.0;
pub const CLOSE_RADIUS: f32 = 4.0;
pub const CLOSE_GLYPH: f32 = 12.0;
/// The Main badge.
pub const BADGE_HEIGHT: f32 = 16.0;
pub const BADGE_PAD_X: f32 = 5.0;
pub const BADGE_GAP: f32 = 3.0;
pub const BADGE_RADIUS: f32 = 4.0;
/// The badge's label, in pt at the 16 px default.
pub const BADGE_TEXT: f32 = 11.0;
pub const BADGE_GLYPH: f32 = 10.0;
/// The gap between a title's chevron and its related count.
pub const CHEVRON_GAP: f32 = 2.0;
/// A title's chevron glyph.
pub const CHEVRON_GLYPH: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the chip's label reads as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    /// A tab in a strip: the 12.5 pt medium label.
    Tab,
    /// A lone chat's title: chrome type, semibold, the window's name.
    Title,
}

/// What one chip draws.
pub struct TabStyle {
    pub label: Label,
    /// The label's colour, already resolved for the selection and the
    /// strip's focus.
    pub text: Hsla,
    /// The selected fill; `None` leaves the background to the host's hover
    /// wash.
    pub fill: Option<Hsla>,
    /// Whether a close slot ends the chip (tighter trailing padding).
    pub closable: bool,
    pub min_width: f32,
    pub max_width: f32,
}

/// One tab or title: its parts in a line, padded and rounded. The host adds
/// the parts, the id and everything interactive.
pub fn tab(style: &TabStyle) -> Div {
    let chip = div()
        .h(px(TAB_HEIGHT))
        .min_w(px(style.min_width))
        .max_w(px(style.max_width))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(PART_GAP))
        .pl(px(PAD_LEFT))
        .pr(px(if style.closable {
            PAD_RIGHT_CLOSABLE
        } else {
            PAD_RIGHT
        }))
        .rounded(px(TAB_RADIUS))
        .cursor_pointer();
    let chip = match style.label {
        Label::Tab => chip
            .text_size(ui_rems(TAB_TEXT))
            .font_weight(FontWeight::MEDIUM),
        Label::Title => chip
            .text_size(ui_rems(TITLE_TEXT))
            .line_height(ui_rems(TITLE_LINE_HEIGHT))
            .font_weight(FontWeight::SEMIBOLD),
    };
    let chip = chip.text_color(style.text);
    match style.fill {
        Some(fill) => chip.bg(fill),
        None => chip,
    }
}

/// The chip's title: it takes the room that is left and truncates.
pub fn title(text: impl Into<SharedString>) -> Div {
    div().min_w_0().truncate().child(text.into())
}

/// The close slot: hidden until the pointer is over the chip's hover
/// `group`, or always shown (the active tab). The host adds the glyph, its
/// hover fill and the click.
pub fn close_slot(group: impl Into<SharedString>, shown: bool) -> Div {
    div()
        .size(px(CLOSE_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CLOSE_RADIUS))
        .opacity(if shown { 1.0 } else { 0.0 })
        .group_hover(group, |style| style.opacity(1.0))
}

/// What the Main badge draws: its wash and its label's colour, already held
/// to the contrast floor over that wash.
pub struct BadgeStyle {
    pub fill: Hsla,
    pub text: Hsla,
}

/// The Main badge's pill. The host adds its glyph and label.
pub fn badge(style: &BadgeStyle) -> Div {
    div()
        .flex_none()
        .h(px(BADGE_HEIGHT))
        .px(px(BADGE_PAD_X))
        .flex()
        .items_center()
        .gap(px(BADGE_GAP))
        .rounded(px(BADGE_RADIUS))
        .bg(style.fill)
        .text_size(ui_rems(BADGE_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(style.text)
}

/// Finished while the user was elsewhere: a small dot in the host's tone.
pub fn unread_dot(color: Hsla) -> Div {
    div()
        .size(px(UNREAD_DOT))
        .flex_none()
        .rounded_full()
        .bg(color)
}

/// A title's menu chevron and its related count. The host adds the glyph
/// and the count.
pub fn title_chevron(text: Hsla) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(CHEVRON_GAP))
        .text_color(text)
}
