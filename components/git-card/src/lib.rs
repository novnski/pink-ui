// PINK's Git card (changes/git_actions.rs `render_git_card`, `git_button`,
// `git_primary_button`, `render_next_git_action`, `render_pr_status`,
// `render_checkpoint_section`; changes/pull_requests.rs `render_pr_panel`):
// the card that opens over the working-tree diff, with the branch and
// remote, the recommended next step, the pull request's status, its panel
// (details and checks, or the draft form), file checkpoints, the changed
// files with their pick boxes, the commit message and the action buttons,
// secondary or wearing the accent plate. UI rethink decision 54: drawn here,
// consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the text, the inputs, the pull
// request badge (`pink-pr-badge`), every colour already resolved (each
// button's plate blended over its hover fade) and the mono family; it adds
// the roles, accessible names, focus rings, scrolling, clicks, links and
// every Git and GitHub operation. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, ElementId, FontWeight, Hsla, IntoElement, SharedString, Stateful, div,
    prelude::*, px, rems,
};

/// The card's height cap, in pt at the 16 px default; it scrolls past it.
pub const CARD_MAX_HEIGHT: f32 = 530.0;
/// The card's side and vertical insets, in fixed pixels (PINK
/// `Theme::SPACE_MD`, `Theme::SPACE_SM`).
pub const CARD_PAD_X: f32 = 12.0;
pub const CARD_PAD_Y: f32 = 8.0;
/// The gap between the card's lines, in pt at the 16 px default.
pub const CARD_GAP: f32 = 6.0;
/// A button's height and side pad, in pt at the 16 px default, and its
/// corner in fixed pixels (PINK `Theme::CONTROL_RADIUS`).
pub const BUTTON_HEIGHT: f32 = 26.0;
pub const BUTTON_PAD_X: f32 = 10.0;
pub const RADIUS: f32 = 6.0;
/// A button's label and the card's line type, in pt at the 16 px default.
pub const BUTTON_TEXT: f32 = 12.0;
pub const LINE_TEXT: f32 = 11.0;
/// The file list's height cap and the commit message's slot, in pt at the
/// 16 px default.
pub const FILE_LIST_MAX_HEIGHT: f32 = 130.0;
pub const MESSAGE_HEIGHT: f32 = 30.0;
/// The checkpoint patch's and the pull request description's height caps,
/// in pt at the 16 px default.
pub const PATCH_MAX_HEIGHT: f32 = 140.0;
pub const DESCRIPTION_MAX_HEIGHT: f32 = 120.0;
/// The pick box glyphs.
pub const PICKED: &str = "\u{2611}";
pub const UNPICKED: &str = "\u{2610}";

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The card's and the panel's plate, resolved by the host.
pub struct CardStyle {
    /// The rule under the card, the panel's edge (PINK `border`).
    pub border: Hsla,
    /// The plate (PINK `card_glass_bg`).
    pub fill: Hsla,
}

/// The card: full width over the diff, its lines `CARD_GAP` apart inside
/// `CARD_PAD_X` / `CARD_PAD_Y`, capped at `CARD_MAX_HEIGHT`, a rule under it.
/// The host adds the vertical scrolling.
pub fn card(id: impl Into<ElementId>, style: &CardStyle) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w_full()
        .max_h(ui_rems(CARD_MAX_HEIGHT))
        .flex()
        .flex_col()
        .gap(ui_rems(CARD_GAP))
        .px(px(CARD_PAD_X))
        .py(px(CARD_PAD_Y))
        .border_b_1()
        .border_color(style.border)
        .bg(style.fill)
}

/// What one button draws, resolved by the host.
pub struct ButtonStyle {
    /// The plate, already blended over the hover fade (PINK
    /// `surface_raised`, or `accent_strong` on the button that performs the
    /// recommended step).
    pub fill: Hsla,
    /// The 1 px edge (transparent on the accent plate).
    pub border: Hsla,
    pub text: Hsla,
    /// The accent plate's label is medium; a secondary one keeps the
    /// inherited weight.
    pub weight: Option<FontWeight>,
}

/// A card button: `label` on a `BUTTON_HEIGHT` plate inside a 1 px edge.
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    style: &ButtonStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(ui_rems(BUTTON_HEIGHT))
        .px(ui_rems(BUTTON_PAD_X))
        .flex()
        .items_center()
        .rounded(px(RADIUS))
        .border_1()
        .border_color(style.border)
        .text_size(ui_rems(BUTTON_TEXT))
        .text_color(style.text)
        .cursor_pointer()
        .when_some(style.weight, |el, weight| el.font_weight(weight))
        .bg(style.fill)
        .child(label.into())
}

/// A row of buttons (or a line's facts and its button), `CARD_GAP` apart.
pub fn button_row() -> Div {
    div().flex().items_center().gap(ui_rems(CARD_GAP))
}

/// The branch and remote, as two facts `8` apart, followed by Refresh.
pub fn facts(color: Hsla) -> Div {
    div()
        .flex()
        .items_center()
        .gap(ui_rems(8.0))
        .text_size(ui_rems(LINE_TEXT))
        .text_color(color)
}

/// A line of the card's text: a notice, a reason, a status.
pub fn note(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_size(ui_rems(LINE_TEXT))
        .text_color(color)
        .child(text.into())
}

/// What the next-step line draws, resolved by the host.
pub struct NextStepStyle {
    /// The step's name (PINK `text`): body text, the accent stays on its
    /// button.
    pub label: Hsla,
    /// Why (PINK `text_dim`).
    pub reason: Hsla,
}

/// The recommended next step: its name in medium weight, then why.
pub fn next_step(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    reason: impl Into<SharedString>,
    style: &NextStepStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_baseline()
        .gap(ui_rems(CARD_GAP))
        .min_w_0()
        .text_size(ui_rems(LINE_TEXT))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.label)
                .child(label.into()),
        )
        .child(
            div()
                .min_w_0()
                .text_color(style.reason)
                .child(reason.into()),
        )
}

/// The pull request's status line; the host adds the badge, the state and
/// the title (`status_state`, `status_title`) or a plain message.
pub fn status_row(color: Hsla) -> Div {
    div()
        .flex()
        .items_center()
        .gap(ui_rems(CARD_GAP))
        .text_size(ui_rems(LINE_TEXT))
        .text_color(color)
}

/// The pull request's state in its tone.
pub fn status_state(text: impl Into<SharedString>, color: Hsla) -> Div {
    div().text_color(color).child(text.into())
}

/// The pull request's title, truncating.
pub fn status_title(text: impl Into<SharedString>) -> Div {
    div().min_w_0().truncate().child(text.into())
}

/// The changed files, `4` apart and capped at `FILE_LIST_MAX_HEIGHT`. The
/// host adds the scrolling.
pub fn file_list(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .max_h(ui_rems(FILE_LIST_MAX_HEIGHT))
        .flex()
        .flex_col()
        .gap(ui_rems(4.0))
}

/// One changed file: its pick box, its path and its Stage / Unstage.
pub fn file_row() -> Div {
    div().flex().items_center().gap(ui_rems(8.0)).min_w_0()
}

/// A file's pick box (`PICKED` or `UNPICKED`).
pub fn pick_box(id: impl Into<ElementId>, picked: bool, color: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .cursor_pointer()
        .text_size(ui_rems(LINE_TEXT))
        .text_color(color)
        .child(if picked { PICKED } else { UNPICKED })
}

/// A file's path in the mono family, taking the row's free width.
pub fn file_path(text: impl Into<SharedString>, mono: SharedString, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .font_family(mono)
        .text_size(ui_rems(LINE_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The commit message's slot.
pub fn message(input: impl IntoElement) -> Div {
    div().h(ui_rems(MESSAGE_HEIGHT)).child(input)
}

/// File checkpoints: a section `5` between its lines with a rule under it.
pub fn section(border: Hsla) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(ui_rems(5.0))
        .py(px(CARD_PAD_Y))
        .border_b_1()
        .border_color(border)
}

/// What a checkpoint line draws, resolved by the host.
pub struct CheckpointStyle {
    /// The time (PINK `text_dim`).
    pub stamp: Hsla,
    /// The outcome (PINK `text_faint`).
    pub outcome: Hsla,
}

/// One checkpoint: when, how it ended, then its Diff / Preview restore.
pub fn checkpoint_line(
    stamp: impl Into<SharedString>,
    outcome: impl Into<SharedString>,
    style: &CheckpointStyle,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(ui_rems(CARD_GAP))
        .text_size(ui_rems(LINE_TEXT))
        .text_color(style.stamp)
        .child(stamp.into())
        .child(div().text_color(style.outcome).child(outcome.into()))
}

/// A stack of lines `4` apart (the checkpoint diff, the restore preview).
pub fn stack() -> Div {
    div().flex().flex_col().gap(ui_rems(4.0))
}

/// The checkpoint's patch in the mono family, capped at
/// `PATCH_MAX_HEIGHT`. The host adds the scrolling.
pub fn patch(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    mono: SharedString,
    color: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .max_h(ui_rems(PATCH_MAX_HEIGHT))
        .text_size(ui_rems(LINE_TEXT))
        .font_family(mono)
        .text_color(color)
        .child(text.into())
}

/// The pull request panel: a bordered plate inside the card, its lines
/// `CARD_GAP` apart.
pub fn panel(id: impl Into<ElementId>, style: &CardStyle) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(ui_rems(CARD_GAP))
        .p(px(CARD_PAD_Y))
        .border_1()
        .border_color(style.border)
        .rounded(px(RADIUS))
        .bg(style.fill)
}

/// The panel's heading, its Close button at the far edge.
pub fn panel_header(title: impl Into<SharedString>, color: Hsla, close: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(ui_rems(BUTTON_TEXT))
                .text_color(color)
                .child(title.into()),
        )
        .child(close)
}

/// What the pull request's title line draws, resolved by the host.
pub struct PrTitleStyle {
    /// The number and the draft tag (PINK `text_dim`).
    pub quiet: Hsla,
    pub title: Hsla,
}

/// The pull request's number, title and, for a draft, its tag.
pub fn pr_title(
    number: impl Into<SharedString>,
    title: impl Into<SharedString>,
    draft: Option<SharedString>,
    style: &PrTitleStyle,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(ui_rems(CARD_GAP))
        .text_size(ui_rems(BUTTON_TEXT))
        .child(
            div()
                .flex_none()
                .text_color(style.quiet)
                .child(number.into()),
        )
        .child(div().min_w_0().text_color(style.title).child(title.into()))
        .when_some(draft, |row, draft| {
            row.child(
                div()
                    .flex_none()
                    .text_size(ui_rems(LINE_TEXT))
                    .text_color(style.quiet)
                    .child(draft),
            )
        })
}

/// The pull request's branches and its state in medium weight.
pub fn pr_refs(refs: impl Into<SharedString>, state: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(ui_rems(CARD_GAP))
        .text_size(ui_rems(LINE_TEXT))
        .text_color(color)
        .child(div().min_w_0().child(refs.into()))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .child(state.into()),
        )
}

/// The pull request's description, capped at `DESCRIPTION_MAX_HEIGHT`.
/// The host adds the scrolling.
pub fn pr_description(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    color: Hsla,
) -> Stateful<Div> {
    div()
        .id(id)
        .max_h(ui_rems(DESCRIPTION_MAX_HEIGHT))
        .text_size(ui_rems(LINE_TEXT))
        .text_color(color)
        .child(text.into())
}

/// What a check row draws, resolved by the host.
pub struct CheckStyle {
    pub name: Hsla,
    /// The status in its outcome's tone.
    pub status: Hsla,
}

/// One reported check: its name, then its status at the far edge.
pub fn check_row(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    status: impl Into<SharedString>,
    style: &CheckStyle,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .gap(ui_rems(8.0))
        .text_size(ui_rems(LINE_TEXT))
        .text_color(style.name)
        .child(div().min_w_0().truncate().child(name.into()))
        .child(div().text_color(style.status).child(status.into()))
}

/// A draft form field: `height` fixed pixels inside a 1 px edge.
pub fn field(height: f32, border: Hsla, input: impl IntoElement) -> Div {
    div()
        .h(px(height))
        .border_1()
        .border_color(border)
        .rounded(px(4.0))
        .child(input)
}
