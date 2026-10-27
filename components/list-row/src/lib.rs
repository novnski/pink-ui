// List row and status indicator for PINK's sidebar (UI rethink milestone 1,
// decisions 54 to 56, novnski/pink#173, geometry after novnski/pink#185).
// The host supplies theme tokens, the Interface scale, glyphs, hover and
// selection state, and actions; this component owns only presentation. It
// has no stored state, timers or app logic. Plain `//` comments: the pink-ui
// gallery compiles this file in place with `include!` for its story.

use gpui::{AnyElement, Div, FontWeight, Hsla, SharedString, Stateful, div, prelude::*, px};

/// One line: Pink shell.rs `compact_chat_row_height`, 32 pt at the 16 px
/// default, scaled with the Interface size.
pub const ONE_LINE_HEIGHT: f32 = 32.0;
/// Two lines: Pink shell.rs `chat_row_height`, a title line and one meta
/// line on the 8 pt rhythm (52 pt, so at least 48).
pub const TWO_LINE_HEIGHT: f32 = 52.0;
/// No target is smaller than this at any Interface size (Pink
/// surface_chrome.rs `MIN_HIT`).
pub const MIN_HIT: f32 = 24.0;
/// From the row's edge to the title and to the time: the row's own padding,
/// equal on both sides so a selected row's text sits balanced in its wash
/// (Pink `Theme::SPACE_SM`). The host places the row on its gutter.
pub const ROW_INSET: f32 = 8.0;
/// The leading mark's slot and the gap after it (Pink shell.rs
/// `SIDEBAR_MARK_SLOT`, `SIDEBAR_MARK_GAP`): one grid for every row kind.
pub const MARK_SLOT: f32 = 16.0;
pub const MARK_GAP: f32 = 8.0;

/// The row's height at an Interface scale for one or two lines.
pub fn row_height(two_lines: bool, scale: f32) -> f32 {
    let base = if two_lines {
        TWO_LINE_HEIGHT
    } else {
        ONE_LINE_HEIGHT
    };
    (base * scale).max(MIN_HIT)
}

/// What a thread is doing, each signal separate (decision 56). Running is
/// never shown or spoken as needing the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// An agent is working.
    Running,
    /// The agent asked the user a question.
    WaitingForInput,
    /// The agent is paused until the user approves something.
    NeedsApproval,
    /// The run failed.
    Failed,
    /// A finished result nobody has read yet.
    Unread,
}

impl Signal {
    /// The signal's spoken name, used in the row's accessible name.
    pub fn spoken(self) -> &'static str {
        match self {
            Self::Running => "Running",
            Self::WaitingForInput => "Waiting for your reply",
            Self::NeedsApproval => "Waiting for your approval",
            Self::Failed => "Failed",
            Self::Unread => "Unread result",
        }
    }

    /// Whether it waits on the user. Running never does.
    pub fn needs_you(self) -> bool {
        matches!(
            self,
            Self::WaitingForInput | Self::NeedsApproval | Self::Failed
        )
    }

    /// The word a waiting or failed signal shows (Pink's status pill).
    pub fn word(self) -> Option<&'static str> {
        match self {
            Self::WaitingForInput => Some("Reply"),
            Self::NeedsApproval => Some("Approve"),
            Self::Failed => Some("Failed"),
            Self::Running | Self::Unread => None,
        }
    }
}

/// Host tokens, already floored for contrast by the host's theme.
pub struct RowStyle {
    /// The Interface size over 16.
    pub scale: f32,
    /// Corner radius of the row's wash (Pink `radius::MD`, 8).
    pub radius: f32,
    /// The title at rest, the title on a selected row, the secondary line.
    pub title: Hsla,
    pub title_selected: Hsla,
    pub secondary: Hsla,
    /// The time and counts.
    pub meta: Hsla,
    /// Row washes: at rest (usually transparent), under the pointer, and
    /// the one selected style.
    pub rest_bg: Hsla,
    pub hover_bg: Hsla,
    pub selected_bg: Hsla,
    /// The trailing corner's fixed width: the time at rest, the actions
    /// under the pointer. Fixed so the title never moves on hover.
    pub corner_width: f32,
    /// Signal tones (text-ready) and the pill's backing alpha.
    pub running: Hsla,
    pub waiting: Hsla,
    pub failed: Hsla,
    pub unread: Hsla,
    pub pill_backing: f32,
}

/// A thread's status (Pink shell.rs `render_chat_row` status): a host
/// glyph or a 6 pt busy square for Running, a 6 pt dot for Unread, and a
/// pill naming the next step for a waiting or failed thread. A one-line
/// (compact) row spends no width on the pill's cloth.
pub fn status_indicator(
    signal: Signal,
    glyph: Option<AnyElement>,
    compact: bool,
    style: &RowStyle,
) -> Div {
    let s = style.scale;
    let tone = match signal {
        Signal::Running => style.running,
        Signal::WaitingForInput | Signal::NeedsApproval => style.waiting,
        Signal::Failed => style.failed,
        Signal::Unread => style.unread,
    };
    let base = div().flex_none().flex().items_center();
    match signal.word() {
        None => base.child(glyph.unwrap_or_else(|| {
            div()
                .size(px(6.0 * s))
                .rounded(px(if signal == Signal::Running {
                    1.5
                } else {
                    3.0 * s
                }))
                .bg(tone)
                .into_any_element()
        })),
        Some(word) => base.child(
            div()
                .h(px(16.0 * s))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(3.0 * s))
                .when(!compact, |pill| pill.px(px(5.0)))
                .rounded(px(4.0))
                .when(!compact, |pill| pill.bg(tone.opacity(style.pill_backing)))
                .text_size(px(11.0 * s))
                .line_height(px(14.0 * s))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tone)
                .children(glyph)
                .child(word),
        ),
    }
}

/// What one row shows. Two lines when it has a secondary line.
pub struct ListRow {
    pub title: SharedString,
    /// The quieter second line (project, branch, harness), host-built.
    pub secondary: Option<AnyElement>,
    /// A mark in the leading slot, before the title (shelf rows).
    pub leading: Option<AnyElement>,
    /// The time-ago in the trailing corner.
    pub time: Option<SharedString>,
    pub status: Option<Signal>,
    /// The host's glyph for the status (a spinner for Running, the glyph
    /// a pill wears).
    pub status_glyph: Option<AnyElement>,
    /// Chips after the title (counts, an open tab), host-built.
    pub chips: Vec<AnyElement>,
    /// Actions shown in the corner under the pointer, in place of the time.
    pub actions: Option<AnyElement>,
    pub selected: bool,
    /// Under the pointer: the host tracks hover and passes it in.
    pub hovered: bool,
}

impl ListRow {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            secondary: None,
            leading: None,
            time: None,
            status: None,
            status_glyph: None,
            chips: Vec::new(),
            actions: None,
            selected: false,
            hovered: false,
        }
    }

    /// The row's accessible name: the title, then its signal's spoken name
    /// ("Retire expired coupon codes, Waiting for your approval"). The host
    /// sets it with its runtime's accessibility API (`role(Button)`,
    /// `aria_label`, `aria_selected`), which crates.io GPUI 0.2.2 lacks.
    pub fn accessible_name(&self) -> SharedString {
        match self.status {
            Some(signal) => format!("{}, {}", self.title, signal.spoken()).into(),
            None => self.title.clone(),
        }
    }
}

/// A sidebar row (Pink shell.rs `render_chat_row` after #181 and #185):
/// 32 pt for one line and 52 pt for two, scaled; the title on the row's
/// title column in every scope (no indent under a group header) and the
/// trailing content ending on the time column, `ROW_INSET` from each edge,
/// so a selected row's text sits balanced in its wash. Hover changes the
/// wash and swaps the time for the actions inside a fixed-width corner, so
/// the title never moves. One selected style. The host adds the row's
/// semantics ([`ListRow::accessible_name`]), hover tracking, click, focus
/// and tooltip to the returned element; it keeps a 24 pt hit area at every
/// Interface size. The title and the
/// corner carry the debug selectors `{id}-title` and `{id}-corner` for
/// geometry checks.
pub fn list_row(id: impl Into<SharedString>, row: ListRow, style: &RowStyle) -> Stateful<Div> {
    let id: SharedString = id.into();
    let (title_selector, corner_selector) = (format!("{id}-title"), format!("{id}-corner"));
    let s = style.scale;
    let two_lines = row.secondary.is_some();
    let compact = !two_lines;
    let bg = if row.selected {
        style.selected_bg
    } else if row.hovered {
        style.hover_bg
    } else {
        style.rest_bg
    };
    let title_color = if row.selected || row.hovered {
        style.title_selected
    } else {
        style.title
    };
    let status = row
        .status
        .map(|signal| status_indicator(signal, row.status_glyph, compact, style));
    let (line_status, meta_status) = if compact {
        (status, None)
    } else {
        (None, status)
    };
    let corner = div()
        .debug_selector(move || corner_selector)
        .flex_none()
        .w(px(style.corner_width))
        .h(px(18.0 * s))
        .flex()
        .items_center()
        .justify_end()
        .text_size(px(11.0 * s))
        .text_color(style.meta)
        .map(|corner| match (row.hovered, row.actions) {
            (true, Some(actions)) => corner.child(actions),
            _ => corner.children(row.time),
        });
    div()
        .id(id)
        .w_full()
        .h(px(row_height(two_lines, s)))
        .flex_none()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.0 * s))
        .px(px(ROW_INSET))
        .rounded(px(style.radius))
        .bg(bg)
        .cursor_pointer()
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(MARK_GAP * s))
                .when_some(row.leading, |line, mark| {
                    line.child(
                        div()
                            .w(px(MARK_SLOT * s))
                            .flex_none()
                            .flex()
                            .justify_start()
                            .child(mark),
                    )
                })
                .child(
                    div()
                        .debug_selector(move || title_selector)
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.0 * s))
                        .line_height(px(17.0 * s))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(title_color)
                        .child(row.title),
                )
                .children(row.chips)
                .children(line_status)
                .child(corner),
        )
        .when_some(row.secondary, |el, secondary| {
            el.child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0 * s))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.0 * s))
                            .line_height(px(14.0 * s))
                            .text_color(style.secondary)
                            .child(secondary),
                    )
                    .children(meta_status),
            )
        })
}
