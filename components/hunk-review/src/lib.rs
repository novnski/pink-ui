// PINK's per-hunk review of an agent's file edit (transcript.rs
// `detail_body`, `ToolDetail::Diff`, novnski/pink#310): a review band under
// each changed block of the inline diff, in the shape of the gallery's
// Inline Diff review card and PINK's inline review comment. A band is an
// `ink(0.05)` strip with a 3 pt bar that firms up while the block holds
// keyboard focus, a 22 pt header naming the block (`path:line` in mono) with
// its state chip and, while focused, the key legend, then the status copy at
// 12/18 and the host's Keep and Drop buttons on the right.
//
// The review model lives here too, so PINK and the gallery word and key a
// review the same way: what the band says for a pending, kept, dropped,
// deciding or refused block, which decisions it offers, the accessible names
// of the band and its actions, the "N of M reviewed" progress, and Y / N /
// J / K. A refusal keeps the recorded state and states the engine's reason;
// nothing reads as dropped until the engine confirms it.
//
// The package draws only. The host supplies the colours already resolved
// (the band fill, the bar, the copy inks, the chip tones) and the copy; it
// adds the ids, roles, accessible names, focus handles and rings, the
// buttons, tooltips and listeners. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its Inline Diff story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The bar on the band's leading edge (the diff rows' accent bar width).
/// Fixed, like every hairline.
pub const BAR_WIDTH: f32 = 3.0;
/// The bar's opacity at rest and while the block holds keyboard focus.
pub const BAR_REST: f32 = 0.35;
pub const BAR_ACTIVE: f32 = 0.7;
/// The band's content padding: horizontal, then top and bottom.
pub const PAD_X: f32 = 16.0;
pub const PAD_Y: f32 = 10.0;
/// The header row: the location, the state chip and the key legend.
pub const HEADER_HEIGHT: f32 = 22.0;
pub const HEADER_GAP: f32 = 6.0;
/// The mono `path:line` location.
pub const LOCATION_TEXT: f32 = 11.0;
/// The status copy under the header.
pub const STATUS_TEXT: f32 = 12.0;
pub const LINE_HEIGHT: f32 = 18.0;
/// The state chip after the location.
pub const CHIP_HEIGHT: f32 = 16.0;
pub const CHIP_PAD_X: f32 = 6.0;
pub const CHIP_RADIUS: f32 = 4.0;
pub const CHIP_TEXT: f32 = 10.5;
/// The actions column: the host's `Sm` buttons, this far apart, inset from
/// the band's trailing edge by the code surface's content inset.
pub const ACTION_HEIGHT: f32 = 24.0;
pub const ACTION_GAP: f32 = 6.0;
pub const ACTIONS_INSET: f32 = 12.0;
/// A key cap in the legend and the gap to its verb.
pub const KEY_HEIGHT: f32 = 16.0;
pub const KEY_PAD_X: f32 = 4.0;
pub const KEY_RADIUS: f32 = 4.0;
pub const KEY_TEXT: f32 = 10.0;
pub const KEY_GAP: f32 = 4.0;
pub const LEGEND_GAP: f32 = 10.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The band's height in design points for `lines` status lines (at least
/// one). Multiply by the Interface size over 16 for pixels.
pub fn height(lines: usize) -> f32 {
    PAD_Y * 2.0 + HEADER_HEIGHT + LINE_HEIGHT * lines.max(1) as f32
}

/// The actions' top offset in design points: centred on the header and the
/// first status line.
pub fn actions_top() -> f32 {
    PAD_Y + (HEADER_HEIGHT + LINE_HEIGHT - ACTION_HEIGHT) / 2.0
}

// ---------------------------------------------------------------------------
// The review model
// ---------------------------------------------------------------------------

/// A block's recorded review state, as the engine returns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockState {
    Pending,
    Kept,
    Dropped,
}

/// A decision the reviewer makes on one block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Keep,
    Drop,
}

impl Decision {
    /// The button label.
    pub fn label(self) -> &'static str {
        match self {
            Decision::Keep => "Keep",
            Decision::Drop => "Drop",
        }
    }

    /// The key that makes this decision.
    pub fn key(self) -> &'static str {
        match self {
            Decision::Keep => "Y",
            Decision::Drop => "N",
        }
    }
}

/// The chip and copy tone a band's status asks for; the host resolves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Pending or in flight: the muted ink.
    Neutral,
    /// Kept: the addition tone.
    Kept,
    /// Dropped: the deletion tone.
    Dropped,
    /// Refused: the danger tone.
    Refused,
}

/// What one block's band shows. The recorded state is always the engine's;
/// a decision in flight or refused never replaces it.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// The host is still reading the block states.
    Reading,
    /// The state the engine returned.
    Settled(BlockState),
    /// A decision was sent and has no answer yet.
    Deciding {
        state: BlockState,
        decision: Decision,
    },
    /// The engine refused the decision and left the file unchanged.
    Refused {
        state: BlockState,
        decision: Decision,
        reason: SharedString,
    },
}

impl Status {
    /// The recorded state, when the host has one.
    pub fn state(&self) -> Option<BlockState> {
        match self {
            Status::Reading => None,
            Status::Settled(state)
            | Status::Deciding { state, .. }
            | Status::Refused { state, .. } => Some(*state),
        }
    }

    /// The state chip's word.
    pub fn chip(&self) -> &'static str {
        match self {
            Status::Reading => "Reading",
            Status::Deciding {
                decision: Decision::Drop,
                ..
            } => "Dropping",
            Status::Deciding {
                decision: Decision::Keep,
                ..
            } => "Keeping",
            Status::Refused { .. } => "Refused",
            Status::Settled(BlockState::Pending) => "Pending",
            Status::Settled(BlockState::Kept) => "Kept",
            Status::Settled(BlockState::Dropped) => "Dropped",
        }
    }

    pub fn tone(&self) -> Tone {
        match self {
            Status::Refused { .. } => Tone::Refused,
            Status::Settled(BlockState::Kept) => Tone::Kept,
            Status::Settled(BlockState::Dropped) => Tone::Dropped,
            _ => Tone::Neutral,
        }
    }

    /// The status line: what is true of the file now, in plain words.
    pub fn headline(&self) -> SharedString {
        match self {
            Status::Reading => "Reading this change's review state".into(),
            Status::Settled(BlockState::Pending) => {
                "Keep the agent's change, or drop it to restore the original lines".into()
            }
            Status::Settled(BlockState::Kept) => "You kept this change".into(),
            Status::Settled(BlockState::Dropped) => {
                "Dropped, so the file has the original lines again. Keep reapplies the change"
                    .into()
            }
            Status::Deciding {
                decision: Decision::Drop,
                ..
            } => "Dropping this change in the file".into(),
            Status::Deciding {
                decision: Decision::Keep,
                state: BlockState::Dropped,
            } => "Reapplying this change in the file".into(),
            Status::Deciding {
                decision: Decision::Keep,
                ..
            } => "Keeping this change".into(),
            Status::Refused {
                decision: Decision::Drop,
                ..
            } => "Couldn't drop this change. The file is unchanged".into(),
            Status::Refused {
                decision: Decision::Keep,
                ..
            } => "Couldn't keep this change. The file is unchanged".into(),
        }
    }

    /// The second line: the engine's reason for a refusal.
    pub fn detail(&self) -> Option<SharedString> {
        match self {
            Status::Refused { reason, .. } => Some(reason.clone()),
            _ => None,
        }
    }

    /// How many status lines the band draws (see [`height`]).
    pub fn lines(&self) -> usize {
        1 + usize::from(self.detail().is_some())
    }

    /// Whether the band offers `decision` now: never while reading or while
    /// a decision is in flight, and never the state the block already has.
    pub fn offers(&self, decision: Decision) -> bool {
        match self {
            Status::Reading | Status::Deciding { .. } => false,
            Status::Settled(state) | Status::Refused { state, .. } => !matches!(
                (state, decision),
                (BlockState::Kept, Decision::Keep) | (BlockState::Dropped, Decision::Drop)
            ),
        }
    }
}

/// A block's location: the file name and the first line it changes.
pub fn location(path: &str, line: usize) -> SharedString {
    let name = path
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(path);
    format!("{name}:{line}").into()
}

/// The band's accessible name: which change, its state and, for a refusal,
/// the reason.
pub fn accessible_name(path: &str, line: usize, status: &Status) -> SharedString {
    let state = match status {
        Status::Reading => "review state loading".to_string(),
        Status::Settled(BlockState::Pending) => "not reviewed".to_string(),
        Status::Settled(BlockState::Kept) => "kept".to_string(),
        Status::Settled(BlockState::Dropped) => "dropped".to_string(),
        Status::Deciding {
            decision: Decision::Drop,
            ..
        } => "dropping".to_string(),
        Status::Deciding {
            decision: Decision::Keep,
            ..
        } => "keeping".to_string(),
        Status::Refused {
            decision, reason, ..
        } => format!(
            "{} refused: {reason}",
            match decision {
                Decision::Keep => "keep",
                Decision::Drop => "drop",
            }
        ),
    };
    format!("Change at {path} line {line}, {state}").into()
}

/// An action's accessible name, naming the change it acts on. Keep on a
/// dropped change is the undo.
pub fn action_name(decision: Decision, path: &str, line: usize, status: &Status) -> SharedString {
    match (decision, status.state()) {
        (Decision::Keep, Some(BlockState::Dropped)) => {
            format!("Keep the change at {path} line {line} again").into()
        }
        (Decision::Keep, _) => format!("Keep the change at {path} line {line}").into(),
        (Decision::Drop, _) => format!("Drop the change at {path} line {line}").into(),
    }
}

/// "2 of 3 reviewed": blocks that are no longer pending, over all of them.
pub fn progress(states: &[BlockState]) -> SharedString {
    let reviewed = states.iter().filter(|s| **s != BlockState::Pending).count();
    format!("{reviewed} of {} reviewed", states.len()).into()
}

/// The engine's refusal reason in plain words: its `edit_review_*:` code
/// and any transport prefix before it go, the first letter is capitalized.
pub fn refusal_reason(error: &str) -> SharedString {
    let text = error.trim();
    let text = match text.find("edit_review_") {
        Some(at) => {
            let coded = &text[at..];
            coded
                .split_once(": ")
                .map(|(_, reason)| reason)
                .unwrap_or(coded)
        }
        None => text,
    };
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()).into(),
        None => "The engine refused without a reason".into(),
    }
}

/// What a key does to the review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Decide(Decision),
    /// Move to the next (`1`) or previous (`-1`) block.
    Step(isize),
}

/// Y keeps, N drops, J and K (or the arrows) move. Modified keys stay with
/// the app (Cmd-N, Ctrl-K).
pub fn key_command(key: &str, platform: bool, control: bool, alt: bool) -> Option<Command> {
    if platform || control || alt {
        return None;
    }
    match key {
        "y" => Some(Command::Decide(Decision::Keep)),
        "n" => Some(Command::Decide(Decision::Drop)),
        "j" | "down" => Some(Command::Step(1)),
        "k" | "up" => Some(Command::Step(-1)),
        _ => None,
    }
}

/// The block a step lands on: clamped to the ends, never wrapping.
pub fn step(current: usize, delta: isize, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    current.saturating_add_signed(delta).min(count - 1)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// What a band draws, already resolved by the host.
pub struct BandStyle {
    /// The band's wash.
    pub fill: Hsla,
    /// The bar, at full opacity; [`bar`] applies the focus firmness.
    pub bar: Hsla,
    /// The `path:line` location.
    pub location: Hsla,
    /// The status line.
    pub headline: Hsla,
    /// The refusal reason.
    pub detail: Hsla,
}

/// The band: `height(lines)` tall, the bar, the content and the actions in
/// a row on the wash. The host adds the [`bar`], the [`content`] and the
/// [`actions`], plus the id, focus handle, role and name.
pub fn band(style: &BandStyle, lines: usize) -> Div {
    div()
        .h(ui_rems(height(lines)))
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .overflow_hidden()
        .bg(style.fill)
}

/// The leading bar, `focus` from 0 (rest) to 1 (the block holds focus).
pub fn bar(style: &BandStyle, focus: f32) -> Div {
    let focus = focus.clamp(0.0, 1.0);
    div().w(px(BAR_WIDTH)).h_full().flex_none().bg(style
        .bar
        .opacity(BAR_REST + (BAR_ACTIVE - BAR_REST) * focus))
}

/// The header over the status lines. The host adds the [`header`] and the
/// [`headline`] (and the [`detail`] for a refusal).
pub fn content() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .px(ui_rems(PAD_X))
        .py(ui_rems(PAD_Y))
}

/// The header row. The host adds the [`location_label`], the [`chip`] and,
/// while the block holds focus, the [`legend`].
pub fn header() -> Div {
    div()
        .h(ui_rems(HEADER_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(HEADER_GAP))
        .overflow_hidden()
}

/// The `path:line` location, 11 pt. The host adds its mono family.
pub fn location_label(style: &BandStyle, text: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(LOCATION_TEXT))
        .text_color(style.location)
        .child(text.into())
}

/// The state chip: the tone's word on a faint plate of the same tone. The
/// host resolves `ink` readable on `plate`.
pub fn chip(ink: Hsla, plate: Hsla, word: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .h(ui_rems(CHIP_HEIGHT))
        .px(ui_rems(CHIP_PAD_X))
        .rounded(px(CHIP_RADIUS))
        .bg(plate)
        .flex()
        .items_center()
        .text_size(ui_rems(CHIP_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(ink)
        .child(word.into())
}

/// One status line at 12/18, truncated: the band's height is analytic.
pub fn headline(style: &BandStyle, text: impl Into<SharedString>) -> Div {
    div()
        .h(ui_rems(LINE_HEIGHT))
        .flex_none()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(STATUS_TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(style.headline)
        .child(text.into())
}

/// The refusal's reason line, in the host's danger ink.
pub fn detail(style: &BandStyle, text: impl Into<SharedString>) -> Div {
    headline(style, text).text_color(style.detail)
}

/// The actions column, centred on the header and the first status line. The
/// host adds its `Sm` Drop and Keep buttons.
pub fn actions() -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_start()
        .gap(ui_rems(ACTION_GAP))
        .pt(ui_rems(actions_top()))
        .pr(ui_rems(ACTIONS_INSET))
}

/// The key legend row ("Y Keep  N Drop  J K Move"). The host adds one
/// [`legend_item`] per key.
pub fn legend() -> Div {
    div()
        .flex_none()
        .ml_auto()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(LEGEND_GAP))
}

/// What a legend key draws, already resolved by the host.
pub struct KeyStyle {
    pub cap: Hsla,
    pub key: Hsla,
    pub verb: Hsla,
}

/// One legend item: the key cap and its verb. The host adds its mono family
/// to the cap when it wants one.
pub fn legend_item(style: &KeyStyle, keys: &'static str, verb: &'static str) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(KEY_GAP))
        .child(
            div()
                .h(ui_rems(KEY_HEIGHT))
                .px(ui_rems(KEY_PAD_X))
                .rounded(px(KEY_RADIUS))
                .bg(style.cap)
                .flex()
                .items_center()
                .text_size(ui_rems(KEY_TEXT))
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.key)
                .child(keys),
        )
        .child(
            div()
                .text_size(ui_rems(LOCATION_TEXT))
                .text_color(style.verb)
                .child(verb),
        )
}

/// The legend's three items, in key order.
pub const LEGEND: [(&str, &str); 3] = [("Y", "Keep"), ("N", "Drop"), ("J K", "Move")];
