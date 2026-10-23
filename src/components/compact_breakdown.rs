//! Compact Breakdown – a total and its parts, laid out like the summary card
//! on pink's Usage page.
//!
//! One settings section card (pink: shell/usage.rs `render_usage_page`, the
//! summary `section_card`). The header names the measure over its caption and
//! carries the period switch (pink's segmented control, as on the Usage
//! page's Period row). The first row holds the total in display type with its
//! change on the period before, over a split bar: pink's 6 px usage meter
//! capsule carrying every part in its series color. Then one row per part,
//! largest first: its identity tile, series dot and name, its share and
//! change, and its amount on the right. Switching the period glides the rows
//! into their new order (pink's sidebar resort glide), and hovering a part on
//! the bar reads it out beside the total, the way the Usage chart's hover
//! readout does.
//!
//! ```ignore
//! CompactBreakdown::new("tokens", "Processed tokens")
//!     .unit("tokens")
//!     .part(BreakdownPart::new("chat", "Chat").icon("icons/app/chat.svg"))
//!     .period(BreakdownPeriod::new("7 days", vec![352e6]).previous(vec![331e6]))
//! ```

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyView, App, Div, ElementId, Entity, FontWeight, Hsla, SharedString, Stateful, Window, div,
    prelude::*, px, rgb, svg,
};

use crate::{
    motion,
    pink::{
        self, ControlSize, FillTier, Pink, Segment, TextStyle, TypeStyleExt, icon_size, layout,
        loaders, radius, space,
    },
    story,
};

pub const NAME: &str = "Compact Breakdown";
pub const DESCRIPTION: &str = "A total and its parts laid out like pink's usage summary – the total in display type over a split bar, one row per part with its series dot, share and change, and a period switch that glides the rows into their new order";

/// The card's width: pink's settings column (720) inside its 32 px gutters.
const WIDTH: f32 = layout::PAGE_MAX_WIDTH - 2. * 32.;
/// The split bar's width: the card's content box inside the row padding.
const BAR_W: f32 = WIDTH - 2. - 2. * layout::ROW_PAD_X;
/// The split bar's capsule (pink: usage.rs `usage_meter`, 6 pt) and the
/// taller strip around it that catches the pointer.
const BAR_H: f32 = 6.;
const BAR_HIT: f32 = 12.;
/// Parts below this share draw nothing on the bar (pink's meter: "nothing
/// drawn below 0.5%, no false stub").
const BAR_MIN_SHARE: f64 = 0.005;
/// A part row's height (pink `card_row` around a 13/18 title and a 12/16
/// description 2 apart, plus its 1 px top hairline): the resort glide moves
/// a row this far per rank.
const ROW_H: f32 = 2. * layout::ROW_PAD_Y + 18. + 2. + 16. + 1.;

const ICON_REASONING: &str = "icons/compact-breakdown/brain.svg";
const ICON_CHAT: &str = "icons/compact-breakdown/chat-round-line.svg";
const ICON_EMBEDDINGS: &str = "icons/compact-breakdown/layers-minimalistic.svg";
const ICON_VISION: &str = "icons/compact-breakdown/eye.svg";
const ICON_SPEECH: &str = "icons/compact-breakdown/volume-loud.svg";

type PeriodHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type Formatter = Rc<dyn Fn(f64) -> String>;

/// One slice of the total.
#[derive(Clone)]
pub struct BreakdownPart {
    pub id: SharedString,
    pub label: SharedString,
    /// The glyph in the row's leading identity tile (an SVG asset path).
    pub icon: Option<SharedString>,
    /// Overrides the series color.
    pub color: Option<Hsla>,
}

impl BreakdownPart {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            color: None,
        }
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// The parts' values over one period, in the order the parts were added.
#[derive(Clone)]
pub struct BreakdownPeriod {
    pub label: SharedString,
    pub values: Vec<f64>,
    /// The same parts one period earlier, for the change readouts.
    pub previous: Option<Vec<f64>>,
}

impl BreakdownPeriod {
    pub fn new(label: impl Into<SharedString>, values: Vec<f64>) -> Self {
        Self {
            label: label.into(),
            values,
            previous: None,
        }
    }

    pub fn previous(mut self, values: Vec<f64>) -> Self {
        self.previous = Some(values);
        self
    }
}

#[derive(IntoElement)]
pub struct CompactBreakdown {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    unit: SharedString,
    parts: Vec<BreakdownPart>,
    periods: Vec<BreakdownPeriod>,
    default_period: usize,
    format: Formatter,
    on_period_change: Option<PeriodHandler>,
}

impl CompactBreakdown {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            unit: "total".into(),
            parts: Vec::new(),
            periods: Vec::new(),
            default_period: 0,
            format: Rc::new(compact_number),
            on_period_change: None,
        }
    }

    /// The caption under the title, e.g. "Production · all workspaces".
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// What the total counts, for the shares: "36.2% of tokens".
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn part(mut self, part: BreakdownPart) -> Self {
        self.parts.push(part);
        self
    }

    pub fn period(mut self, period: BreakdownPeriod) -> Self {
        self.periods.push(period);
        self
    }

    pub fn default_period(mut self, ix: usize) -> Self {
        self.default_period = ix;
        self
    }

    /// Formats amounts (default pink's compact counts: "61.2M", "1.46B").
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    pub fn on_period_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_period_change = Some(Rc::new(handler));
        self
    }
}

struct State {
    period: usize,
    /// The part under the pointer on the split bar.
    hovered: Option<usize>,
}

impl RenderOnce for CompactBreakdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let default_period = self.default_period;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State {
                period: default_period,
                hovered: None,
            },
        );
        let (period_ix, hovered) = {
            let s = state.read(cx);
            (
                s.period.min(self.periods.len().saturating_sub(1)),
                s.hovered,
            )
        };

        // --- Header: title, caption and the period switch ---------------
        let switch = (self.periods.len() > 1).then(|| {
            let state = state.clone();
            let handler = self.on_period_change.clone();
            pink::segmented(
                format!("{key}-period"),
                self.periods
                    .iter()
                    .enumerate()
                    .map(|(ix, p)| Segment::new(ix.to_string(), p.label.clone()))
                    .collect(),
                period_ix,
                segmented_width(self.periods.len()),
                ControlSize::Md,
                &t,
                move |ix: &usize, window, cx| {
                    let ix = *ix;
                    let changed = state.update(cx, |s, cx| {
                        if s.period == ix {
                            return false;
                        }
                        s.period = ix;
                        s.hovered = None;
                        cx.notify();
                        true
                    });
                    if changed && let Some(f) = &handler {
                        f(ix, window, cx);
                    }
                },
            )
        });
        // pink: settings/widgets.rs `section_header`, the switch as its
        // trailing action.
        let header = pink::section_header(self.title.clone(), self.subtitle.clone(), &t)
            .children(switch.map(|s| pink::row_controls().child(s)));
        let card = pink::section_card(&t)
            .mt_0()
            .w(px(WIDTH))
            .font_family(pink::SANS)
            .child(header);
        let Some(period) = self.periods.get(period_ix) else {
            return card;
        };

        let count = self.parts.len();
        let value_of = |i: usize| period.values.get(i).copied().unwrap_or(0.).max(0.);
        let total: f64 = (0..count).map(value_of).sum();
        let share_of = |i: usize| {
            if total > 0. { value_of(i) / total } else { 0. }
        };
        let change_of = |i: usize| {
            period
                .previous
                .as_ref()
                .and_then(|p| p.get(i).copied())
                .filter(|p| *p > 0.)
                .map(|p| value_of(i) / p - 1.)
        };
        let tones: Vec<Hsla> = (0..count)
            .map(|i| self.parts[i].color.unwrap_or_else(|| series_tone(i, &t)))
            .collect();
        // Largest first; a part with nothing this period has no row (pink
        // skips providers with no usage).
        let mut order: Vec<usize> = (0..count).filter(|&i| value_of(i) > 0.).collect();
        order.sort_by(|a, b| value_of(*b).total_cmp(&value_of(*a)));

        // --- Total: the figure, its readout and the split bar -----------
        let previous_total = period
            .previous
            .as_ref()
            .map(|p| p.iter().take(count).map(|v| v.max(0.)).sum::<f64>())
            .filter(|p| *p > 0.);
        let readout: Option<String> = match hovered.filter(|i| order.contains(i)) {
            Some(i) => Some(format!(
                "{} · {} · {}",
                self.parts[i].label,
                (self.format)(value_of(i)),
                usage_percent(share_of(i))
            )),
            None => previous_total.map(|p| total_change(total / p - 1.)),
        };
        let drawn: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&i| share_of(i) >= BAR_MIN_SHARE)
            .collect();
        let drawn_total: f64 = drawn.iter().map(|&i| share_of(i)).sum();
        let bar = split_bar(
            &key,
            drawn
                .iter()
                .map(|&i| {
                    (
                        i,
                        self.parts[i].id.clone(),
                        share_of(i) / drawn_total,
                        tones[i],
                    )
                })
                .collect(),
            &state,
            &t,
        );
        // pink: usage.rs summary, the total row (Display, forced semibold).
        let total_row = pink::card_row(false, &t)
            .child(
                div()
                    .min_w_0()
                    .type_style(TextStyle::Display)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child((self.format)(total)),
            )
            .when_some(readout, |row, text| {
                // pink: usage.rs chart row's hover readout (11/14 in
                // `readable_muted(0.85)`), swapping like the working line's
                // status (`STATUS_SWAP`).
                let id = loaders::label_id(&format!("{key}-readout"), &text);
                row.child(
                    pink::row_controls().child(motion::status_swap(
                        id,
                        div()
                            .type_style(TextStyle::Caption)
                            .text_color(t.readable_muted(0.85))
                            .child(text),
                    )),
                )
            })
            .child(pink::row_block().child(bar));

        // --- One row per part, gliding into its new rank ------------------
        let rows = order.iter().enumerate().map(|(rank, &i)| {
            let part = &self.parts[i];
            let shown = motion::follow(
                &format!("{key}-rank-{}", part.id),
                rank as f32,
                motion::RESORT,
            );
            let share = format!("{} of {}", usage_percent(share_of(i)), self.unit);
            let detail = match change_of(i) {
                Some(g) => format!("{share} · {}", part_change(g)),
                None => share,
            };
            part_row(part, tones[i], detail, (self.format)(value_of(i)), &t)
                .relative()
                .top(px((shown - rank as f32) * ROW_H))
        });

        // Followers started above need frames even when nothing else animates.
        motion::keep_alive(window);
        card.child(total_row).children(rows)
    }
}

/// A part's row on the summary card (pink: usage.rs summary provider row):
/// the identity tile, the series dot beside the name over the share line,
/// and the amount at 13/18 medium on the right.
fn part_row(part: &BreakdownPart, tone: Hsla, detail: String, amount: String, t: &Pink) -> Div {
    pink::card_row(false, t)
        .when_some(part.icon.clone(), |row, icon| row.child(part_tile(icon, t)))
        .child(
            div()
                .flex_1()
                .min_w(px(layout::ROW_TEXT_MIN))
                .flex()
                .flex_col()
                .gap(px(space::S2))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(space::S6))
                        .child(div().flex_none().size(px(8.)).rounded_full().bg(tone))
                        .child(pink::row_title(part.label.clone(), t)),
                )
                .child(pink::row_description(detail, t)),
        )
        .child(
            pink::row_controls().child(
                div()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(amount),
            ),
        )
}

/// A part's identity tile (pink: usage.rs `provider_tile`): the row tile's
/// 28 pt frame, radius 7, `border` over `fill(Quaternary)`, with the 16 pt
/// glyph at full text strength so it reads as identity.
fn part_tile(icon: SharedString, t: &Pink) -> Div {
    div()
        .flex_none()
        .size(px(28.))
        .rounded(px(radius::TILE))
        .border_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(icon)
                .flex_none()
                .size(px(icon_size::MD))
                .text_color(t.text),
        )
}

/// The split bar: pink's 6 pt usage meter capsule (usage.rs
/// `usage_meter_with`) carrying the parts in rank order, each in its series
/// color with 2 px between them, on the meter's `fill(Secondary)` track when
/// there is nothing to split. `segments` are `(part, id, share, tone)` with
/// the shares summing to one. Hovering a part reads it out beside the total.
fn split_bar(
    key: &str,
    segments: Vec<(usize, SharedString, f64, Hsla)>,
    state: &Entity<State>,
    t: &Pink,
) -> Stateful<Div> {
    let n = segments.len();
    let avail = BAR_W - space::S2 * n.saturating_sub(1) as f32;
    div()
        .id(SharedString::from(format!("{key}-bar")))
        .w_full()
        .h(px(BAR_HIT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S2))
        .when(n == 0, |bar| {
            bar.child(
                div()
                    .w_full()
                    .h(px(BAR_H))
                    .rounded_full()
                    .bg(t.fill(FillTier::Secondary)),
            )
        })
        .children(
            segments
                .into_iter()
                .enumerate()
                .map(|(pos, (i, id, share, tone))| {
                    let state = state.clone();
                    div()
                        .id(SharedString::from(format!("{key}-bar-{id}")))
                        .flex_none()
                        .w(px(avail * share as f32))
                        .h_full()
                        .flex()
                        .items_center()
                        .on_hover(move |hovered, _, cx| {
                            state.update(cx, |s, cx| {
                                let next = match (*hovered, s.hovered) {
                                    (true, _) => Some(i),
                                    (false, Some(h)) if h == i => None,
                                    (false, other) => other,
                                };
                                if next != s.hovered {
                                    s.hovered = next;
                                    cx.notify();
                                }
                            })
                        })
                        .child(
                            div()
                                .w_full()
                                .h(px(BAR_H))
                                .bg(tone)
                                .when(pos == 0, |d| d.rounded_l(px(BAR_H / 2.)))
                                .when(pos + 1 == n, |d| d.rounded_r(px(BAR_H / 2.))),
                        )
                }),
        )
}

/// A part's series color: pink's usage series (usage.rs `provider_tone`:
/// `text`, then its blue and coral), then the project hues green, amber,
/// violet and pink (`sidebar_projects::PROJECT_HUES`), floored for marks.
fn series_tone(index: usize, t: &Pink) -> Hsla {
    match index {
        0 => t.text,
        1 => rgb(0x5b9bbd).into(),
        2 => rgb(0xd97757).into(),
        n => t.readable_icon(pink::project_tint([1, 2, 0, 4][(n - 3) % 4])),
    }
}

/// pink `usage.rs` widths for its segmented rows: 150 for two options, 216
/// for three, 300 for four; 75 per option past that.
fn segmented_width(count: usize) -> f32 {
    match count {
        0..=2 => 150.,
        3 => 216.,
        4 => 300.,
        n => 75. * n as f32,
    }
}

/// A share as pink's Usage page writes it (usage.rs `usage_percent`):
/// one decimal, "<0.1%" for slivers.
fn usage_percent(share: f64) -> String {
    let percent = share * 100.;
    if percent > 0. && percent < 0.1 {
        "<0.1%".into()
    } else {
        format!("{percent:.1}%")
    }
}

/// A part's change on the period before: "up 25%", "down 6%", "no change".
fn part_change(growth: f64) -> String {
    let pct = (growth.abs() * 100.).round();
    if pct < 1. {
        "no change".into()
    } else if growth > 0. {
        format!("up {pct:.0}%")
    } else {
        format!("down {pct:.0}%")
    }
}

/// The total's change, as the readout beside it.
fn total_change(growth: f64) -> String {
    let pct = (growth.abs() * 100.).round();
    if pct < 1. {
        "Level with the period before".into()
    } else if growth > 0. {
        format!("Up {pct:.0}% on the period before")
    } else {
        format!("Down {pct:.0}% on the period before")
    }
}

/// pink's compact counts (usage.rs `compact_tokens`): K/M/B/T with two
/// decimals under 10, one under 100 and none above, trailing zeros trimmed;
/// smaller counts grouped by thousands.
fn compact_number(value: f64) -> String {
    let value = value.max(0.).round();
    for (threshold, suffix) in [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")] {
        if value >= threshold {
            let scaled = value / threshold;
            let digits = if scaled >= 100. {
                0
            } else if scaled >= 10. {
                1
            } else {
                2
            };
            let formatted = format!("{scaled:.digits$}");
            let trimmed = if formatted.contains('.') {
                formatted.trim_end_matches('0').trim_end_matches('.')
            } else {
                &formatted
            };
            return format!("{trimmed}{suffix}");
        }
    }
    // pink: usage.rs `count`.
    let digits = (value as u64).to_string();
    let mut grouped = String::new();
    for (ix, ch) in digits.chars().enumerate() {
        if ix > 0 && (digits.len() - ix) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        CompactBreakdown::new("compact-breakdown", "Tokens by workload")
            .subtitle("Production · all workspaces")
            .unit("tokens")
            .part(BreakdownPart::new("reasoning", "Reasoning").icon(ICON_REASONING))
            .part(BreakdownPart::new("chat", "Chat").icon(ICON_CHAT))
            .part(BreakdownPart::new("embeddings", "Embeddings").icon(ICON_EMBEDDINGS))
            .part(BreakdownPart::new("vision", "Vision").icon(ICON_VISION))
            .part(BreakdownPart::new("speech", "Speech").icon(ICON_SPEECH))
            .period(
                BreakdownPeriod::new("Past 24h", vec![61.2e6, 48.9e6, 27.3e6, 9.8e6, 4.1e6])
                    .previous(vec![52.0e6, 50.3e6, 25.1e6, 7.1e6, 4.4e6]),
            )
            .period(
                BreakdownPeriod::new("7 days", vec![318e6, 352e6, 186e6, 61e6, 30e6])
                    .previous(vec![254e6, 331e6, 179e6, 44e6, 29e6]),
            )
            .period(
                BreakdownPeriod::new("30 days", vec![0.88e9, 1.46e9, 0.92e9, 0.19e9, 0.21e9])
                    .previous(vec![0.61e9, 1.38e9, 0.85e9, 0.12e9, 0.20e9]),
            )
            .default_period(1)
    })
}
