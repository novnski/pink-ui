// Tool activity: a verb-first sentence per call or group (UI rethink D63),
// its disclosure header, and the failure line under a failed call. The host
// supplies the sentence, the lifecycle, theme tokens, icons, motion and
// actions. This component owns only presentation.
use gpui::{
    AnyElement, Div, ElementId, FontWeight, Hsla, SharedString, Stateful, div, prelude::*, px,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityState {
    Running,
    Thinking,
    NeedsApproval,
    NeedsInput,
    Recorded,
    Stopped,
    NeedsReview,
}

impl ActivityState {
    /// The visible state word, only for states the sentence cannot carry.
    /// Running and thinking read in the sentence's own verb ("Running
    /// `pwd`"), and a settled record shows no state word at all: "Recorded"
    /// never meant the user's goal succeeded.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Running | Self::Thinking | Self::Recorded => None,
            Self::NeedsApproval => Some("Needs approval"),
            Self::NeedsInput => Some("Needs input"),
            Self::Stopped => Some("Stopped"),
            Self::NeedsReview => Some("Needs review"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActivitySummary {
    pub state: ActivityState,
    /// Failed executable calls the sentence does not already name. A host
    /// whose sentence is itself the failure ("Failed to run `pwd`") passes
    /// zero, so the header never says it twice.
    pub failures: usize,
}

impl ActivitySummary {
    pub fn failure_label(self) -> Option<String> {
        (self.failures > 0).then(|| format!("{} failed", self.failures))
    }

    /// The accessible name: the visible sentence, then any failure count and
    /// state word, in the order they are drawn.
    pub fn description(self, sentence: &str) -> String {
        let mut parts = vec![sentence.to_owned()];
        parts.extend(self.failure_label());
        parts.extend(self.state.label().map(str::to_owned));
        parts.join(", ")
    }
}

/// One run of a sentence. Text is the verb and connecting words; Code is a
/// command, pattern or tool name drawn as inline code; Object is a file,
/// page or other noun in the primary tone. Code and Object shrink with an
/// ellipsis when the line is short; Text never does, so the verb stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(SharedString),
    Code(SharedString),
    Object(SharedString),
}

pub struct SentenceStyle {
    pub scale: f32,
    pub text_size: f32,
    pub line_height: f32,
    /// Verb and connecting words; `None` inherits the host's colour, so a
    /// hover-blended control brightens them.
    pub verb: Option<Hsla>,
    /// Objects: paths, pages, counts.
    pub object: Hsla,
    pub code: Hsla,
    pub code_fill: Hsla,
    pub code_font: SharedString,
    pub code_size: f32,
}

/// Inline code height inside the sentence's line box.
pub const CODE_HEIGHT_FRACTION: f32 = 0.9;
/// Inline code horizontal padding at the 16 px default.
pub const CODE_PAD_X: f32 = 4.0;
/// Inline code corner radius at the 16 px default.
pub const CODE_RADIUS: f32 = 4.0;
/// Gap between a sentence's runs at the 16 px default.
pub const SEGMENT_GAP: f32 = 4.0;

/// A single-line sentence: runs laid out left to right on one line box, the
/// verb fixed and the objects truncating. The host may replace the leading
/// text run's element (a shimmer) by drawing over it.
pub fn sentence(segments: &[Segment], style: &SentenceStyle) -> Div {
    let s = style.scale;
    let line = style.line_height;
    div()
        .min_w_0()
        .h(px(line))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SEGMENT_GAP * s))
        .overflow_hidden()
        .text_size(px(style.text_size))
        .line_height(px(line))
        .children(segments.iter().map(|segment| match segment {
            Segment::Text(text) => div()
                .flex_none()
                .whitespace_nowrap()
                .when_some(style.verb, |run, verb| run.text_color(verb))
                .child(text.clone())
                .into_any_element(),
            Segment::Object(text) => div()
                .min_w_0()
                .truncate()
                .text_color(style.object)
                .child(text.clone())
                .into_any_element(),
            Segment::Code(text) => div()
                .min_w_0()
                .h(px(line * CODE_HEIGHT_FRACTION))
                .flex()
                .items_center()
                .px(px(CODE_PAD_X * s))
                .rounded(px(CODE_RADIUS * s))
                .bg(style.code_fill)
                .overflow_hidden()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(style.code_font.clone())
                        .text_size(px(style.code_size))
                        .line_height(px(line * CODE_HEIGHT_FRACTION))
                        .text_color(style.code)
                        .child(text.clone()),
                )
                .into_any_element(),
        }))
}

/// Host tokens keep this component inside the existing design system.
pub struct HeaderStyle {
    pub scale: f32,
    pub height: f32,
    pub radius: f32,
    pub left_inset: f32,
    pub text_size: f32,
    pub line_height: f32,
    pub foreground: Hsla,
    pub muted: Hsla,
    pub active: Hsla,
    pub danger: Hsla,
    pub warning: Hsla,
}

/// A stable-height disclosure. Only the sentence shrinks; failures and an
/// attention state remain visible. The host adds semantics, focus and action
/// handlers to the returned control, using the same identity as its detail
/// disclosure, and names it with [`ActivitySummary::description`].
pub fn activity_header(
    id: impl Into<ElementId>,
    title: AnyElement,
    glyph: AnyElement,
    chevron: AnyElement,
    summary: ActivitySummary,
    style: HeaderStyle,
) -> Stateful<Div> {
    let s = style.scale;
    div()
        .id(id)
        .rounded(px(style.radius))
        .max_w_full()
        .min_w_0()
        .h(px(style.height))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0 * s))
        .pl(px(style.left_inset))
        .pr(px(8.0 * s))
        .cursor_pointer()
        .text_size(px(style.text_size))
        .line_height(px(style.line_height))
        .child(glyph)
        .child(
            div()
                .min_w_0()
                .h(px(style.line_height))
                .flex()
                .items_center()
                .overflow_hidden()
                .text_color(style.foreground)
                .child(title),
        )
        .when_some(summary.failure_label(), |row, label| {
            row.child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(style.danger)
                    .child(label),
            )
        })
        .when_some(summary.state.label(), |row, label| {
            row.child(
                div()
                    .flex_none()
                    .text_color(match summary.state {
                        ActivityState::Running | ActivityState::Thinking => style.active,
                        ActivityState::Recorded => style.muted,
                        _ => style.warning,
                    })
                    .child(label),
            )
        })
        .child(chevron)
}

pub struct FailureStyle {
    pub scale: f32,
    pub height: f32,
    pub text_size: f32,
    pub line_height: f32,
    pub message: Hsla,
}

/// The line under a failed call: what the tool reported, then the host's
/// recovery actions. The message truncates; the actions never do.
pub fn failure_line(message: SharedString, actions: Vec<AnyElement>, style: &FailureStyle) -> Div {
    let s = style.scale;
    div()
        .h(px(style.height))
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0 * s))
        .text_size(px(style.text_size))
        .line_height(px(style.line_height))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(style.message)
                .child(message),
        )
        .children(actions)
}

pub struct RecoveryStyle {
    pub scale: f32,
    pub height: f32,
    pub radius: f32,
    pub text: Hsla,
}

/// A text action on the failure line. The host adds the listener, role,
/// accessible name, focus ring and hover fill.
pub fn recovery_action(
    id: impl Into<ElementId>,
    label: SharedString,
    style: &RecoveryStyle,
) -> Stateful<Div> {
    let s = style.scale;
    div()
        .id(id)
        .flex_none()
        .h(px(style.height))
        .px(px(6.0 * s))
        .flex()
        .items_center()
        .rounded(px(style.radius))
        .cursor_pointer()
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
        .child(label)
}
