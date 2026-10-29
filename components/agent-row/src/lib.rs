// PINK's agent row (shell/agents.rs `render_agents_section`): one agent in
// the Agents list, and each row of the gallery's Agent Tree. Two lines, no
// outlined box: the status mark (a 7 pt dot, or the host's working spinner
// while it runs), the title and a quiet status word; then, under the title,
// what the agent is doing beside its figures (model, tokens, tool uses,
// runtime) in a trailing column that ends on one edge. The activity gives
// way first; the figures truncate at half the line, so a long list of
// figures never hides what the agent is doing.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours, the mark,
// the copy and any trailing control on the title line (the gallery's fold
// tile); it adds the ids, roles and accessible names, focus, hover and
// selection fills, an outline's indent and the actions. Plain `//` comments:
// the pink-ui gallery compiles this file in place with `include!` for its
// story.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, relative, rems,
};

// The row, in pt at the 16 px default. The corners stay fixed.
pub const PAD_X: f32 = 10.0;
pub const PAD_Y: f32 = 7.0;
pub const RADIUS: f32 = 8.0;
// Between the title line and the meta line, and between rows of a list.
pub const LINE_GAP: f32 = 2.0;
pub const LIST_GAP: f32 = 2.0;

// The status mark's slot, and the gap after it (and between the meta
// line's activity and figures).
pub const MARK: f32 = 7.0;
pub const LEAD_GAP: f32 = 8.0;
// The meta line starts under the title.
pub const META_INDENT: f32 = MARK + LEAD_GAP;

pub const TITLE_TEXT: f32 = 13.0;
pub const STATUS_TEXT: f32 = 11.0;
pub const META_TEXT: f32 = 11.5;
// The figures' widest share of the meta line.
pub const FIGURES_MAX: f32 = 0.5;
// GPUI's default line height, set explicitly so a row's height is known:
// 2 * PAD_Y + (TITLE_TEXT + META_TEXT) * LINE_HEIGHT + LINE_GAP.
pub const LINE_HEIGHT: f32 = 1.618_034;

/// A row's height at the 16 px default; it scales with the rem size.
pub const ROW_HEIGHT: f32 =
    2.0 * PAD_Y + TITLE_TEXT * LINE_HEIGHT + LINE_GAP + META_TEXT * LINE_HEIGHT;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The resolved colours of a row's text.
pub struct RowStyle {
    /// The title; the host passes the danger tone for a failed agent.
    pub title: Hsla,
    pub status: Hsla,
    pub activity: Hsla,
    pub figures: Hsla,
}

/// What a row says.
pub struct RowText {
    pub title: SharedString,
    pub status: SharedString,
    /// What it is doing, or its outcome.
    pub activity: SharedString,
    /// Its figures, already joined by the host; none for no column.
    pub figures: Option<SharedString>,
}

/// The row: padded and rounded, its two lines `LINE_GAP` apart. The host
/// adds the lines ([`title_line`], [`meta_line`]) and its id, role, label,
/// hover and selection fills, focus ring, indent and actions.
pub fn row() -> Div {
    div()
        .flex_none()
        .px(ui_rems(PAD_X))
        .py(ui_rems(PAD_Y))
        .rounded(px(RADIUS))
        .flex()
        .flex_col()
        .gap(ui_rems(LINE_GAP))
}

/// The status dot.
pub fn dot(color: Hsla) -> Div {
    div()
        .size(ui_rems(MARK))
        .flex_none()
        .rounded_full()
        .bg(color)
}

/// Line one: the host's mark ([`dot`] or a spinner) centred in its slot,
/// the title (`color`; the host passes the danger tone for a failed agent),
/// which truncates, and the host's [`status_word`] (which it may wrap in a
/// swap transition). The host may add a trailing control after it.
pub fn title_line(
    color: Hsla,
    mark: impl IntoElement,
    title: impl Into<SharedString>,
    status: impl IntoElement,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(ui_rems(LEAD_GAP))
        .child(
            div()
                .flex_none()
                .w(ui_rems(MARK))
                .flex()
                .justify_center()
                .child(mark),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(ui_rems(TITLE_TEXT))
                .line_height(relative(LINE_HEIGHT))
                .text_color(color)
                .child(title.into()),
        )
        .child(status)
}

/// The quiet status word: 11 pt medium.
pub fn status_word(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(STATUS_TEXT))
        .line_height(relative(LINE_HEIGHT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text.into())
}

/// Line two, under the title, holding the activity, which gives way first.
/// The host appends the [`figures`] column.
pub fn meta_line(color: Hsla, activity: impl Into<SharedString>) -> Div {
    div()
        .pl(ui_rems(META_INDENT))
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(LEAD_GAP))
        .overflow_hidden()
        .text_size(ui_rems(META_TEXT))
        .line_height(relative(LINE_HEIGHT))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(color)
                .child(activity.into()),
        )
}

/// The meta line's trailing figures: they keep their width while the
/// activity gives way, up to `FIGURES_MAX` of the line, where they truncate
/// so the activity always keeps the rest. A host whose GPUI has font
/// features may add tabular figures.
pub fn figures(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .max_w(relative(FIGURES_MAX))
        .truncate()
        .text_color(color)
        .child(text.into())
}

/// The whole row: [`row`] holding [`title_line`] and [`meta_line`] with its
/// [`figures`].
pub fn agent_row(style: &RowStyle, mark: impl IntoElement, text: RowText) -> Div {
    row()
        .child(title_line(
            style.title,
            mark,
            text.title,
            status_word(style.status, text.status),
        ))
        .child(
            meta_line(style.activity, text.activity).when_some(text.figures, |line, text| {
                line.child(figures(style.figures, text))
            }),
        )
}
