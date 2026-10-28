// PINK's code block chrome (markdown/render.rs `code_block_frame`,
// `code_block_header`, `code_icon_action` and `copy_action`, and
// transcript.rs `code_surface_header`): the rounded frame a fenced code
// block, a generated diagram or a tool's output sits in, its header strip
// with the language on the left and the actions on the right, the
// medium-weight variant of that strip over a tool's output, invocation or
// inline diff, and the two action shapes (a square glyph button, and the
// Copy button that grows a "Copied" label). UI rethink decision 54: drawn
// here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the body, the language text,
// each glyph (its own icon assets, sized with `ACTION_ICON` or
// `COPY_ICON`), each colour already resolved and each action's wash already
// blended over its hover fade; it adds the button role, accessible name,
// focus ring, tooltip, listeners, the copy and wrap behaviour and the
// syntax-highlighted lines. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, AnyElement, Div, ElementId, FontWeight, Hsla, SharedString, Stateful, div,
    prelude::*, px, rems,
};

/// The frame's corner radius, in fixed pixels (PINK `radius::LG`).
pub const FRAME_RADIUS: f32 = 10.0;
/// The header strip's height, in fixed pixels.
pub const HEADER_HEIGHT: f32 = 28.0;
/// The header's lead before the language, in fixed pixels (PINK
/// `CODE_PADDING_X`, the code's own inset).
pub const HEADER_PAD_LEFT: f32 = 12.0;
/// The header's trailing inset after the last action, in fixed pixels.
pub const HEADER_PAD_RIGHT: f32 = 5.0;
/// The language label's text size, in pt at the 16 px default.
pub const LABEL_TEXT: f32 = 11.0;
/// The code surface label's weight (tool output, invocations and diffs).
pub const SURFACE_LABEL_WEIGHT: FontWeight = FontWeight::MEDIUM;
/// The gap between header actions, in fixed pixels.
pub const ACTION_GAP: f32 = 2.0;
/// An action's height (and a glyph action's square), in fixed pixels.
pub const ACTION_SIZE: f32 = 22.0;
/// An action's corner radius, in fixed pixels (PINK `radius::SM`).
pub const ACTION_RADIUS: f32 = 6.0;
/// A glyph action's icon, in fixed pixels.
pub const ACTION_ICON: f32 = 13.0;
/// The Copy action's horizontal padding, in fixed pixels.
pub const COPY_PAD_X: f32 = 6.0;
/// The gap between the Copy action's glyph and its label, in fixed pixels.
pub const COPY_GAP: f32 = 4.0;
/// The Copy action's glyph, in fixed pixels.
pub const COPY_ICON: f32 = 12.0;
/// The Copy action's label size, in pt at the 16 px default.
pub const COPY_TEXT: f32 = 11.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The frame's colours.
#[derive(Clone, Copy, Debug)]
pub struct FrameStyle {
    /// The frame's fill (PINK: `ink(0.035)`).
    pub fill: Hsla,
    /// The frame's 1 px edge and the header's rule (PINK: `theme.border`).
    pub border: Hsla,
    /// The header strip's fill (PINK: `ink(0.02)`).
    pub header_fill: Hsla,
    /// The language label (PINK: `content_muted`).
    pub label: Hsla,
}

/// The frame: full width, `FRAME_RADIUS` corners, a 1 px edge, clipping its
/// body, then the header (when the host has one) above the host's body.
/// Positioned, so the host can overlay a scrollbar.
pub fn frame(id: impl Into<ElementId>, header: Option<Div>, style: FrameStyle) -> Stateful<Div> {
    div()
        .id(id)
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .rounded(px(FRAME_RADIUS))
        .bg(style.fill)
        .border_1()
        .border_color(style.border)
        .overflow_hidden()
        .relative()
        .children(header)
}

/// The strip both headers share: `HEADER_HEIGHT` tall over a 1 px rule on
/// the header fill, `HEADER_PAD_LEFT` in and `pad_right` from the edge, its
/// children centred and pushed to either end.
fn strip(pad_right: f32, style: FrameStyle) -> Div {
    div()
        .h(px(HEADER_HEIGHT))
        .flex_none()
        .pl(px(HEADER_PAD_LEFT))
        .pr(px(pad_right))
        .border_b_1()
        .border_color(style.border)
        .bg(style.header_fill)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
}

/// The actions, `ACTION_GAP` apart, at the strip's trailing end.
fn action_row(actions: Vec<AnyElement>) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(ACTION_GAP))
        .children(actions)
}

/// The header strip: `HEADER_HEIGHT` tall over a 1 px rule, the `language`
/// (if any) on the left and the `actions` on the right, `ACTION_GAP` apart.
/// `None` when there is neither, so a bare block has no strip.
pub fn header(
    language: Option<SharedString>,
    actions: Vec<AnyElement>,
    style: FrameStyle,
) -> Option<Div> {
    if language.is_none() && actions.is_empty() {
        return None;
    }
    Some(
        strip(HEADER_PAD_RIGHT, style)
            .child(
                div()
                    .min_w_0()
                    .text_size(ui_rems(LABEL_TEXT))
                    .text_color(style.label)
                    .children(language),
            )
            .child(action_row(actions)),
    )
}

/// The code surface header over a tool's output, invocation or inline diff
/// (PINK `code_surface_header`): the same strip with a `LABEL_TEXT`
/// `SURFACE_LABEL_WEIGHT` `style.label` caption. The `label` is the host's
/// element ("Output", "Command", or a diff's file name and counts) and the
/// `actions` trail it. The size, weight and colour sit on the strip, so a
/// Copy pill's "Copied" reads in the same weight. With no actions the label
/// keeps `HEADER_PAD_LEFT` from both edges, so a long one truncates at the
/// code's own inset; the host sets the strip's font family.
pub fn surface_header(label: impl IntoElement, actions: Vec<AnyElement>, style: FrameStyle) -> Div {
    let pad_right = if actions.is_empty() {
        HEADER_PAD_LEFT
    } else {
        HEADER_PAD_RIGHT
    };
    strip(pad_right, style)
        .text_size(ui_rems(LABEL_TEXT))
        .font_weight(SURFACE_LABEL_WEIGHT)
        .text_color(style.label)
        .child(label)
        .when(!actions.is_empty(), |strip| {
            strip.child(action_row(actions))
        })
}

/// A glyph action: `glyph` centred in an `ACTION_SIZE` square on the host's
/// `wash` (transparent at rest, or a held tint while toggled on).
pub fn icon_action(id: impl Into<ElementId>, glyph: AnyElement, wash: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(ACTION_SIZE))
        .rounded(px(ACTION_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(wash)
        .child(glyph)
}

/// The Copy action: `glyph` then, once copied, its `label`, `COPY_GAP`
/// apart in an `ACTION_SIZE`-tall pill on the host's `wash`, in `color`.
pub fn copy_action(
    id: impl Into<ElementId>,
    glyph: AnyElement,
    label: Option<SharedString>,
    color: Hsla,
    wash: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(ACTION_SIZE))
        .px(px(COPY_PAD_X))
        .rounded(px(ACTION_RADIUS))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(COPY_GAP))
        .cursor_pointer()
        .bg(wash)
        .text_size(ui_rems(COPY_TEXT))
        .text_color(color)
        .child(glyph)
        .children(label)
}
