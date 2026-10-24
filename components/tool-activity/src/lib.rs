//! Tool activity disclosure. The host supplies factual event counts, lifecycle,
//! theme tokens, icons, motion and actions. This component owns only presentation.
use gpui::{AnyElement, Div, ElementId, FontWeight, Hsla, Stateful, div, prelude::*, px};

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
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::Thinking => "Thinking",
            Self::NeedsApproval => "Needs approval",
            Self::NeedsInput => "Needs input",
            Self::Recorded => "Recorded",
            Self::Stopped => "Stopped",
            Self::NeedsReview => "Needs review",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActivitySummary {
    pub state: ActivityState,
    /// Executable calls only, never reasoning segments or inferred phases.
    pub calls: usize,
    pub reasoning: bool,
    pub running: usize,
    pub failures: usize,
}

impl ActivitySummary {
    pub fn status_label(self) -> String {
        if self.state == ActivityState::Running && self.running > 1 {
            format!("{} active", self.running)
        } else {
            self.state.label().into()
        }
    }

    pub fn count_label(self) -> String {
        format!(
            "{} {}",
            self.calls,
            if self.calls == 1 { "call" } else { "calls" }
        )
    }

    /// Full metadata for the host's accessible disclosure name.
    pub fn description(self) -> String {
        let mut parts = vec![self.status_label()];
        if self.calls > 0 {
            parts.push(self.count_label());
        }
        if self.reasoning && self.calls > 0 {
            parts.push("includes reasoning".into());
        }
        if self.failures > 0 {
            parts.push(format!("{} failed", self.failures));
        }
        parts.join(", ")
    }
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

/// A stable-height disclosure. Only the primary label shrinks; lifecycle and
/// failures remain visible. The host adds semantics, focus and action handlers
/// to the returned control, using the same identity as its detail disclosure.
pub fn activity_header(
    id: impl Into<ElementId>,
    title: AnyElement,
    glyph: AnyElement,
    chevron: AnyElement,
    summary: ActivitySummary,
    style: HeaderStyle,
) -> Stateful<Div> {
    let s = style.scale;
    let status_color = match summary.state {
        ActivityState::Running | ActivityState::Thinking => style.active,
        ActivityState::Stopped
        | ActivityState::NeedsReview
        | ActivityState::NeedsApproval
        | ActivityState::NeedsInput => style.warning,
        ActivityState::Recorded => style.muted,
    };
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
                .flex_shrink(1.0)
                .h(px(style.line_height))
                .flex()
                .items_center()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.foreground)
                .child(title),
        )
        .when(summary.calls > 0, |row| {
            row.child(
                div()
                    .flex_none()
                    .text_color(style.muted)
                    .child(summary.count_label()),
            )
        })
        .child(
            div()
                .flex_none()
                .text_color(status_color)
                .child(summary.status_label()),
        )
        .when(summary.failures > 0, |row| {
            row.child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(style.danger)
                    .child(format!("{} failed", summary.failures)),
            )
        })
        .child(chevron)
}
