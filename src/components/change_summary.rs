//! Change Summary – what moved since last time, one metric per row with its
//! trend, on a Pink settings section card.
//!
//! One section card (Pink: settings/widgets.rs `section_card`) under a
//! `section_header`, with the period as a quiet badge in the header's
//! trailing slot (Pink `widgets::badge`). Each metric is a `card_row`: a trend
//! tile in the row tile's frame, washed like Pink's status pills (good in
//! `success`, bad in `danger`, level in the muted tone, all at 0.12) around
//! an up or down arrow or a level mark, then the metric's name over what
//! happened in words. The color says good or bad, the glyph says which way;
//! the copy itself stays muted ("one colored glyph, never a colored row").
//! Rows rest without hover like Pink's settings rows, and arrive one after
//! another on Pink's tool-row reveal.
//!
//! ```ignore
//! ChangeSummary::new("weekly", vec![
//!     MetricChange::new("Active users", "Up 6.1% to 18.2K").tone(Tone::Good),
//!     MetricChange::new("Support tickets", "Up 14% to 312").tone(Tone::Bad).direction(Direction::Up),
//! ])
//! .period("This week")
//! ```

use gpui::{
    AnyView, App, Div, ElementId, IntoElement, SharedString, Window, div, prelude::*, px, svg,
};

use crate::{
    motion,
    pink::{self, NON_TEXT_CONTRAST_TARGET, Pink, icon_size, layout, radius},
    story,
};

pub const NAME: &str = "Change Summary";
pub const DESCRIPTION: &str = "What moved since last time on a Pink section card – one metric per row, its name over what happened, beside a tinted tile whose arrow says which way and whose color says good or bad";

/// The card's widest: a compact data card (pink-ui's stage size for it).
const MAX_WIDTH: f32 = 480.;
/// The trend tile: Pink's settings row tile (widgets.rs `row_tile`, 28 pt).
const TILE: f32 = 28.;
/// Status washes (Pink: shell.rs sidebar status pill, `tone @ 0.12`).
const TONE_WASH: f32 = 0.12;
/// Arriving rows rise this far (Pink: transcript.rs row reveal).
const ROW_RISE: f32 = 4.;

const ICON_UP: &str = "icons/change-summary/arrow-up.svg";
const ICON_DOWN: &str = "icons/change-summary/arrow-down.svg";
const ICON_LEVEL: &str = "icons/change-summary/minus.svg";

/// How a change reads: good is a green arrow, bad a red one, neutral a muted
/// level mark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    Good,
    Bad,
    #[default]
    Neutral,
}

/// Which way the trend arrow points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// One metric row.
#[derive(Clone, Debug)]
pub struct MetricChange {
    pub label: SharedString,
    /// What happened, in words, e.g. "Up 6.1% to 18.2K".
    pub change: SharedString,
    pub tone: Tone,
    /// Defaults to up for good and neutral, down for bad.
    pub direction: Option<Direction>,
}

impl MetricChange {
    pub fn new(label: impl Into<SharedString>, change: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            change: change.into(),
            tone: Tone::Neutral,
            direction: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = Some(direction);
        self
    }

    fn arrow(&self) -> Direction {
        self.direction.unwrap_or(if self.tone == Tone::Bad {
            Direction::Down
        } else {
            Direction::Up
        })
    }

    /// The tile's glyph: a level mark for neutral changes, else the arrow.
    fn glyph(&self) -> &'static str {
        match (self.tone, self.arrow()) {
            (Tone::Neutral, _) => ICON_LEVEL,
            (_, Direction::Up) => ICON_UP,
            (_, Direction::Down) => ICON_DOWN,
        }
    }
}

#[derive(IntoElement)]
pub struct ChangeSummary {
    id: ElementId,
    title: SharedString,
    period: Option<SharedString>,
    changes: Vec<MetricChange>,
}

impl ChangeSummary {
    pub fn new(id: impl Into<ElementId>, changes: Vec<MetricChange>) -> Self {
        Self {
            id: id.into(),
            title: "What's changed".into(),
            period: None,
            changes,
        }
    }

    /// The heading, "What's changed" by default.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The span the changes cover, e.g. "This week", shown as a badge at the
    /// end of the heading.
    pub fn period(mut self, period: impl Into<SharedString>) -> Self {
        self.period = Some(period.into());
        self
    }
}

impl RenderOnce for ChangeSummary {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();

        // Pink: settings/widgets.rs `section_header`, the period as its
        // trailing badge (`widgets::badge`) on the title's 20 pt line.
        let header = pink::section_header(self.title.clone(), None, &t).when_some(
            self.period.clone(),
            |header, period| {
                header.child(
                    pink::row_controls()
                        .child(pink::badge(period, pink::Tone::Neutral, &t).line_height(px(14.))),
                )
            },
        );

        let rows = self.changes.iter().enumerate().map(|(ix, change)| {
            // Pink: settings/widgets.rs `card_row` with its identity tile and
            // `row_text`; the header above takes the first hairline.
            let row = pink::card_row(false, &t)
                .child(trend_tile(change, &t))
                .child(pink::row_text(
                    change.label.clone(),
                    Some(change.change.clone()),
                    &t,
                ));
            // Pink: transcript.rs new rows, 360 ms expo-out after a 90 ms
            // beat, 65 ms apart, rising 4 pt.
            let delay = motion::tool_row_arrival_delay(ix, true).as_millis() as u64;
            motion::rise_in(
                ElementId::NamedInteger(format!("{key}-row-in").into(), ix as u64),
                motion::TOOL_ROW_REVEAL.with_delay(delay),
                ROW_RISE,
                row,
            )
        });

        pink::section_card(&t)
            .id(self.id.clone())
            .mt_0()
            .w_full()
            .max_w(px(MAX_WIDTH))
            .font_family(pink::SANS)
            .child(header)
            .children(rows)
    }
}

/// The trend tile: Pink's settings row tile frame (widgets.rs `row_tile`: 28
/// pt, radius 7, a 16 pt glyph) washed like its status pills (shell.rs
/// sidebar status pill: `tone @ 0.12`, the glyph floored on that wash).
fn trend_tile(change: &MetricChange, t: &Pink) -> Div {
    let tone = match change.tone {
        Tone::Good => t.success,
        Tone::Bad => t.danger,
        Tone::Neutral => t.text_muted,
    };
    let wash = tone.opacity(TONE_WASH);
    div()
        .flex_none()
        .size(px(TILE))
        .rounded(px(radius::TILE))
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(change.glyph())
                .flex_none()
                .size(px(icon_size::MD))
                .text_color(t.readable_on(tone, wash, NON_TEXT_CONTRAST_TARGET)),
        )
}

/// The demo's weekly metrics: two good rises, a good fall, a bad rise and a
/// level one.
pub fn demo_changes() -> Vec<MetricChange> {
    vec![
        MetricChange::new("Active users", "Up 6.1% to 18.2K").tone(Tone::Good),
        MetricChange::new("Activation", "Climbed to 41%, up 3 pts").tone(Tone::Good),
        MetricChange::new("Churn", "Down to 2.1% from 2.6%")
            .tone(Tone::Good)
            .direction(Direction::Down),
        MetricChange::new("Support tickets", "Up 14% to 312")
            .tone(Tone::Bad)
            .direction(Direction::Up),
        MetricChange::new("Median session", "Level at 6m 40s"),
    ]
}

/// A deploy's read-out: a good fall, a bad rise, a bad fall and a level one.
fn deploy_changes() -> Vec<MetricChange> {
    vec![
        MetricChange::new("p95 latency", "Down to 212 ms from 248 ms")
            .tone(Tone::Good)
            .direction(Direction::Down),
        MetricChange::new("Error rate", "Up to 0.42% from 0.31%")
            .tone(Tone::Bad)
            .direction(Direction::Up),
        MetricChange::new("Checkout conversion", "Down 0.4 pts to 3.1%").tone(Tone::Bad),
        MetricChange::new("Bundle size", "Unchanged at 412 KB"),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .w(px(MAX_WIDTH))
            .flex()
            .flex_col()
            .gap(px(layout::SECTION_GAP))
            .child(ChangeSummary::new("change-summary", demo_changes()).period("This week"))
            .child(
                ChangeSummary::new("change-summary-deploy", deploy_changes())
                    .title("Since the last deploy"),
            )
    })
}
