// PINK's Agent Plan (issue #370, part of #266): the steps an agent proposes
// before it runs, shaped by the person, then followed as they run. The plan
// rides on the composer column. In draft it is a tray tucked behind the
// composer (the message queue's tray and rows, `pink-queue-row`) under a
// header bar drawn like the goal card's (composer/goals.rs): a step's title
// over its tool and estimate, and a quiet "Add a step" row. Once it runs it
// is the provider checklist (composer/checklist.rs): one "2 of 5, In
// progress: ..." disclosure row beside a close tile, unfolding into one
// "Status: step" line per step and a closing note. A failed step's notice
// is `pink-notice`'s danger chip.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours (hover
// fills already blended), the glyph paths, the copy and the step data; it
// adds the ids, roles and accessible names, focus rings, tooltips, the tray
// and row actions (`pink-queue-row`), drag and drop, scrolling, the text
// fields and every action (rename, reorder, add, remove, Run plan, Stop,
// retry, skip). Plain `//` comments: the pink-ui gallery compiles this file
// in place with `include!` for its story.

use std::ops::Range;

use gpui::{
    AbsoluteLength, Div, FontWeight, HighlightStyle, Hsla, SharedString, StyledText, div,
    prelude::*, relative, rems, svg,
};

// The composer column, in pt at the 16 px default: the stack's gap, and
// the tray's side inset and the part of it tucked behind the composer
// (PINK `QUEUE_SIDE_INSET`, `QUEUE_COMPOSER_OVERLAP`).
pub const STACK_GAP: f32 = 8.0;
pub const TRAY_INSET: f32 = 16.0;
pub const TRAY_OVERLAP: f32 = 18.0;

// The tray's header bar (goals.rs `HEADER_HEIGHT`, `GLYPH`): the glyph, the
// medium "Plan", the objective, then the facts in a quieter 12.
pub const HEADER_H: f32 = 38.0;
pub const HEADER_TEXT: f32 = 13.0;
pub const HEADER_FACTS_TEXT: f32 = 12.0;
pub const HEADER_GLYPH: f32 = 14.0;
pub const HEADER_PAD_L: f32 = 14.0;
pub const HEADER_PAD_R: f32 = 6.0;
pub const HEADER_GAP: f32 = 7.0;
// The trailing controls (Run plan) sit in this padding.
pub const CONTROLS_PAD_R: f32 = 8.0;
pub const CONTROLS_GAP: f32 = 2.0;

// One step in the tray: the queue row's 36 pt height (`pink-queue-row`
// `ROW_HEIGHT`), its title over the tool and estimate.
pub const ROW_H: f32 = 36.0;
pub const TITLE_TEXT: f32 = 12.5;
pub const TITLE_LINE: f32 = 16.0;
pub const DETAIL_TEXT: f32 = 11.0;
pub const DETAIL_LINE: f32 = 13.0;
pub const LINE_GAP: f32 = 1.0;
// The add row's leading glyph slot (the queue's drag marker slot).
pub const SLOT_W: f32 = 20.0;
pub const SLOT_H: f32 = 28.0;
pub const SLOT_GLYPH: f32 = 13.0;

// The running plan (checklist.rs `render_checklist`): 11.5 text, a 28
// disclosure row with a 12 chevron, a 24 close tile, at most 144 of step
// lines.
pub const CHECKLIST_TEXT: f32 = 11.5;
pub const CHECKLIST_ROW_H: f32 = 28.0;
pub const CHECKLIST_PAD_X: f32 = 8.0;
pub const CHECKLIST_GAP: f32 = 6.0;
pub const CHECKLIST_RADIUS: f32 = 6.0;
pub const CHEVRON: f32 = 12.0;
pub const CLOSE: f32 = 24.0;
pub const LINES_MAX_H: f32 = 144.0;
pub const LINES_PAD_X: f32 = 12.0;
pub const LINES_GAP: f32 = 4.0;
pub const NOTE_PAD_Y: f32 = 4.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// Where a step is in the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

impl StepStatus {
    /// The checklist's status word.
    pub fn word(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Running => "In progress",
            Self::Done => "Completed",
            Self::Failed => "Failed",
            Self::Skipped => "Skipped",
        }
    }
}

/// The running plan's one-line summary: how many steps are done, the step
/// it is on, and the range of the one red word when a step failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub done: usize,
    pub total: usize,
    pub label: String,
    pub red: Option<Range<usize>>,
}

/// Summarises a run from each step's status and title, in plan order:
/// "2 of 5, In progress: step", then "Failed: step", "Next: step", "Plan
/// complete" once every step is done, or "Plan finished" when some were
/// skipped.
pub fn summary<'a>(steps: impl IntoIterator<Item = (StepStatus, &'a str)>) -> Summary {
    let steps: Vec<_> = steps.into_iter().collect();
    let total = steps.len();
    let done = steps
        .iter()
        .filter(|(status, _)| *status == StepStatus::Done)
        .count();
    let find = |wanted: StepStatus| {
        steps
            .iter()
            .find(|(status, _)| *status == wanted)
            .map(|(_, title)| *title)
    };
    let (detail, failed) = if let Some(title) = find(StepStatus::Running) {
        (format!("In progress: {title}"), false)
    } else if let Some(title) = find(StepStatus::Failed) {
        (format!("Failed: {title}"), true)
    } else if let Some(title) = find(StepStatus::Pending) {
        (format!("Next: {title}"), false)
    } else if done == total {
        ("Plan complete".to_owned(), false)
    } else {
        ("Plan finished".to_owned(), false)
    };
    let prefix = format!("{done} of {total}, ");
    let red = failed.then(|| prefix.len()..prefix.len() + "Failed".len());
    Summary {
        done,
        total,
        label: format!("{prefix}{detail}"),
        red,
    }
}

/// `text` with its `red` range in `danger`: the run's one red word.
pub fn status_text(
    text: impl Into<SharedString>,
    red: Option<Range<usize>>,
    danger: Hsla,
) -> StyledText {
    let text = text.into();
    let styled = StyledText::new(text.clone());
    match red.filter(|range| range.end <= text.len()) {
        Some(range) => styled.with_highlights([(
            range,
            HighlightStyle {
                color: Some(danger),
                ..Default::default()
            },
        )]),
        None => styled,
    }
}

/// The header bar's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct HeaderStyle {
    pub glyph: Hsla,
    pub label: Hsla,
    pub objective: Hsla,
    pub facts: Hsla,
}

/// The tray's header bar: a `HEADER_H` strip. The host puts the
/// [`header_summary`] in it, then the [`controls`] with Run plan.
pub fn header() -> Div {
    div()
        .h(ui_rems(HEADER_H))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
}

/// The header's summary, which takes the bar's free width: the glyph, the
/// medium `label` ("Plan"), the objective (which alone truncates) and the
/// trailing facts ("5 steps", "about 1m"), each apart on the row's gap
/// rather than joined by a separator. The host adds the status role and
/// its accessible name.
pub fn header_summary(
    style: &HeaderStyle,
    glyph: impl Into<SharedString>,
    label: impl Into<SharedString>,
    objective: impl Into<SharedString>,
    facts: impl IntoIterator<Item = SharedString>,
) -> Div {
    let objective: SharedString = objective.into();
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .pl(ui_rems(HEADER_PAD_L))
        .pr(ui_rems(HEADER_PAD_R))
        .flex()
        .items_center()
        .gap(ui_rems(HEADER_GAP))
        .text_size(ui_rems(HEADER_TEXT))
        .child(
            svg()
                .path(glyph.into())
                .flex_none()
                .size(ui_rems(HEADER_GLYPH))
                .text_color(style.glyph),
        )
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.label)
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_color(style.objective)
                .when(!objective.is_empty(), |el| el.child(objective)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(ui_rems(HEADER_GAP))
                .text_size(ui_rems(HEADER_FACTS_TEXT))
                .text_color(style.facts)
                .children(facts.into_iter().map(|fact| div().child(fact))),
        )
}

/// The header's trailing controls, where the host puts Run plan.
pub fn controls() -> Div {
    div()
        .flex_none()
        .h_full()
        .pr(ui_rems(CONTROLS_PAD_R))
        .flex()
        .items_center()
        .gap(ui_rems(CONTROLS_GAP))
}

/// A step's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct StepStyle {
    pub title: Hsla,
    pub detail: Hsla,
}

/// A step's text column inside its queue row: the title line (a
/// [`step_title`], which the host may wrap to catch a double click for
/// rename) over an optional detail line (the tool and estimate, or what the
/// provider did not share). It takes the row's free width.
pub fn step_text(style: &StepStyle, title: impl IntoElement, detail: Option<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(ui_rems(LINE_GAP))
        .child(title)
        .when_some(detail, |el, detail| {
            el.child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(ui_rems(DETAIL_TEXT))
                    .line_height(ui_rems(DETAIL_LINE))
                    .text_color(style.detail)
                    .child(detail),
            )
        })
}

/// A step's title line, cut with an ellipsis.
pub fn step_title(color: Hsla, title: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(ui_rems(TITLE_TEXT))
        .line_height(ui_rems(TITLE_LINE))
        .text_color(color)
        .child(title.into())
}

/// The column that holds a step's rename field, or the add row's field.
pub fn field(color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .text_size(ui_rems(TITLE_TEXT))
        .line_height(ui_rems(TITLE_LINE))
        .text_color(color)
}

/// The add row's leading glyph in its `SLOT_W` by `SLOT_H` slot, so the
/// label starts on the steps' title column.
pub fn slot(glyph: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .w(ui_rems(SLOT_W))
        .h(ui_rems(SLOT_H))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(glyph.into())
                .flex_none()
                .size(ui_rems(SLOT_GLYPH))
                .text_color(color),
        )
}

/// The add row's quiet label ("Add a step"), in a colour the host blends
/// over the row's hover.
pub fn add_label(color: Hsla, label: impl Into<SharedString>) -> Div {
    div()
        .text_size(ui_rems(TITLE_TEXT))
        .line_height(ui_rems(TITLE_LINE))
        .text_color(color)
        .child(label.into())
}

/// The running plan's resolved colours.
#[derive(Clone, Copy, Debug)]
pub struct ChecklistStyle {
    pub text: Hsla,
    pub danger: Hsla,
}

/// The running plan's column: 11.5 muted text. The host adds the
/// [`checklist_row`], then [`lines`] and [`note`] while it is unfolded.
pub fn checklist(style: &ChecklistStyle) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .min_w_0()
        .text_size(ui_rems(CHECKLIST_TEXT))
        .line_height(relative(1.618_034))
        .text_color(style.text)
}

/// The disclosure row's frame: the host puts the [`disclosure`] (made a
/// button) and the [`close`] tile (made a button) in it.
pub fn checklist_row() -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(ui_rems(4.0))
        .min_w_0()
}

/// The disclosure: the chevron at `chevron` and the summary, its failed
/// word in red. It takes the row's free width; the host adds the id, the
/// button role, the hover fill and the toggle.
pub fn disclosure(
    style: &ChecklistStyle,
    chevron: impl Into<SharedString>,
    summary: &Summary,
) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(ui_rems(CHECKLIST_GAP))
        .px(ui_rems(CHECKLIST_PAD_X))
        .h(ui_rems(CHECKLIST_ROW_H))
        .rounded(ui_rems(CHECKLIST_RADIUS))
        .child(
            svg()
                .path(chevron.into())
                .flex_none()
                .size(ui_rems(CHEVRON))
                .text_color(style.text),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(status_text(
                    summary.label.clone(),
                    summary.red.clone(),
                    style.danger,
                )),
        )
}

/// The `CLOSE` tile that stops or closes the run and hands the plan back.
pub fn close(style: &ChecklistStyle, glyph: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .size(ui_rems(CLOSE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(ui_rems(CHECKLIST_RADIUS))
        .child(
            svg()
                .path(glyph.into())
                .flex_none()
                .size(ui_rems(CHEVRON))
                .text_color(style.text),
        )
}

/// The unfolded step lines' column, at most `LINES_MAX_H` tall. The host
/// adds the id, the list role and the scroller.
pub fn lines() -> Div {
    div()
        .w_full()
        .max_h(ui_rems(LINES_MAX_H))
        .flex()
        .flex_col()
        .px(ui_rems(LINES_PAD_X))
        .gap(ui_rems(LINES_GAP))
}

/// One step's line: "Status: step", the failed status word in red. It wraps
/// rather than hiding a long step.
pub fn line(style: &ChecklistStyle, status: StepStatus, title: &str) -> Div {
    let word = status.word();
    let text = format!("{word}: {title}");
    let red = (status == StepStatus::Failed).then_some(0..word.len());
    div()
        .w_full()
        .min_w_0()
        .child(status_text(text, red, style.danger))
}

/// The closing note under the lines ("Agent plan running for 1m").
pub fn note(text: impl Into<SharedString>) -> Div {
    div()
        .px(ui_rems(LINES_PAD_X))
        .py(ui_rems(NOTE_PAD_Y))
        .child(text.into())
}

/// The step slot under a pointer `rel_y` pt into the step list, each slot
/// `slot` tall (PINK terminal/panel.rs `drop_index`).
pub fn drop_index(rel_y: f32, slot: f32, count: usize) -> usize {
    if count == 0 || slot <= 0.0 {
        return 0;
    }
    ((rel_y / slot).floor().max(0.0) as usize).min(count - 1)
}

/// How many slots a row shifts while the row at `from` is dragged over
/// `over`: the rows between them slide one slot toward the gap it leaves
/// (PINK terminal/panel.rs `slide_offset`).
pub fn slide_offset(ix: usize, from: usize, over: usize) -> f32 {
    if from < over && ix > from && ix <= over {
        -1.0
    } else if over < from && ix >= over && ix < from {
        1.0
    } else {
        0.0
    }
}

/// A row's slide, in slots, from where it was (`prev_over`) to where it
/// goes (`over`): the dragged row travels to the hovered slot, the rows in
/// its path slide into the space it left (PINK `queue_drag_offsets`).
/// Multiply by the row height.
pub fn drag_offsets(ix: usize, from: usize, prev_over: usize, over: usize) -> (f32, f32) {
    if ix == from {
        (prev_over as f32 - from as f32, over as f32 - from as f32)
    } else {
        (
            slide_offset(ix, from, prev_over),
            slide_offset(ix, from, over),
        )
    }
}

/// Moves the step at `from` to `to` (clamped), the reorder a drop makes.
/// Returns whether anything moved.
pub fn move_step<T>(steps: &mut Vec<T>, from: usize, to: usize) -> bool {
    if from == to || from >= steps.len() {
        return false;
    }
    let step = steps.remove(from);
    let to = to.min(steps.len());
    steps.insert(to, step);
    true
}
