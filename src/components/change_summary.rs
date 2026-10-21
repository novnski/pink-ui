//! Change Summary – What moved since last time – one metric per row with a colored trend
//!
//! Clean: one white card with hairline shadows; the heading is its first row
//! and every metric sits under a 0.5px line with a small tinted trend mark.
//! Classic: the heading sits in a gray tray and the metrics in a white card
//! on top of it. Rows rise in one after another and tint on hover.
//!
//! ```ignore
//! ChangeSummary::new("weekly", vec![
//!     MetricChange::new("Active users", "Up 6.1% to 18.2K").tone(Tone::Good),
//!     MetricChange::new("Support tickets", "Up 14% to 312").tone(Tone::Bad).direction(Direction::Up),
//! ])
//! .period("This week")
//! ```

use gpui::{
    AnyView, App, BoxShadow, ElementId, FontWeight, Hsla, IntoElement, Pixels, SharedString,
    Window, div, prelude::*, px, relative, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Change Summary";
pub const DESCRIPTION: &str =
    "What moved since last time – one metric per row with a colored trend";

/// How a change reads: good is a green arrow, bad a red one, neutral a gray dash.
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
}

/// The `--ui-cs-*` / `--ui-csc-*` tokens.
struct Palette {
    /// Classic only: the gray tray around the heading and card.
    tray: Option<Hsla>,
    card: Hsla,
    card_shadow: Vec<BoxShadow>,
    /// Classic draws a 1px border around the card.
    card_border: Option<Hsla>,
    line: Hsla,
    title: Hsla,
    title_icon: Hsla,
    period: Hsla,
    label: Hsla,
    change: Hsla,
    row_hover: Hsla,
    good: (Hsla, Hsla),
    bad: (Hsla, Hsla),
    neutral: (Hsla, Hsla),
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => Palette {
                tray: None,
                card: hex(0xffffff),
                card_shadow: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                card_border: None,
                line: black_a(0.06),
                title: hex(0x000000),
                title_icon: black_a(0.447),
                period: black_a(0.45),
                label: black_a(0.88),
                change: black_a(0.61),
                row_hover: black_a(0.015),
                good: (hexa(0x1a9a52, 0.08), hex(0x1a9a52)),
                bad: (hexa(0xe5484d, 0.08), hex(0xe5484d)),
                neutral: (black_a(0.04), black_a(0.447)),
            },
            (Look::Clean, true) => Palette {
                tray: None,
                card: hex(0x1c1c1e),
                card_shadow: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                card_border: None,
                line: white_a(0.08),
                title: hex(0xffffff),
                title_icon: white_a(0.5),
                period: white_a(0.46),
                label: white_a(0.9),
                change: white_a(0.62),
                row_hover: white_a(0.025),
                good: (hexa(0x3ec27a, 0.14), hex(0x3ec27a)),
                bad: (hexa(0xe5484d, 0.14), hex(0xf2777b)),
                neutral: (white_a(0.08), white_a(0.5)),
            },
            (Look::Classic, false) => Palette {
                tray: Some(hex(0xf0f0f0)),
                card: hex(0xffffff),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
                card_border: Some(hex(0xe5e5e5)),
                line: hex(0xf0f0f0),
                title: hex(0x2f6bff),
                title_icon: hex(0x2f6bff),
                period: hex(0x737373),
                label: hex(0x171717),
                change: hex(0x737373),
                row_hover: hex(0xfafafa),
                good: (hex(0xedfaf3), hex(0x1f8a4c)),
                bad: (hex(0xfff0f0), hex(0xe5484d)),
                neutral: (hex(0xf5f5f5), hex(0x8f8f8f)),
            },
            (Look::Classic, true) => Palette {
                tray: Some(hex(0x161618)),
                card: hex(0x232326),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                card_border: Some(white_a(0.08)),
                line: white_a(0.08),
                title: hex(0x6b95ff),
                title_icon: hex(0x6b95ff),
                period: hex(0x8f8f8f),
                label: hex(0xededed),
                change: hex(0x8f8f8f),
                row_hover: hex(0x2a2a2d),
                good: (hexa(0x1f8a4c, 0.18), hex(0x3fbf74)),
                bad: (hexa(0xe5484d, 0.16), hex(0xf2777b)),
                neutral: (hex(0x2a2a2d), hex(0x8f8f8f)),
            },
        }
    }

    fn tone(&self, tone: Tone) -> (Hsla, Hsla) {
        match tone {
            Tone::Good => self.good,
            Tone::Bad => self.bad,
            Tone::Neutral => self.neutral,
        }
    }
}

/// Sizes that differ between the two stylesheets.
struct Metrics {
    text_size: Pixels,
    row_height: Pixels,
    row_pad: Pixels,
    /// Row separators: a 0.5px hairline (Clean, drawn as one device pixel)
    /// or a 1px line (Classic).
    rule: Pixels,
    trend_radius: Pixels,
    /// Corner radius the last row's hover tint follows inside the card.
    inner_radius: Pixels,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                text_size: px(13.),
                row_height: px(48.),
                row_pad: px(16.),
                rule: px(0.5),
                trend_radius: px(5.),
                inner_radius: px(10.),
            },
            Look::Classic => Metrics {
                text_size: px(14.),
                row_height: px(52.),
                row_pad: px(20.),
                rule: px(1.),
                trend_radius: px(6.),
                inner_radius: px(15.),
            },
        }
    }
}

/// A CSS `box-shadow` list for GPUI, which paints the layers the other way
/// round (CSS puts the first on top). Each layer is already converted by
/// `primitives::shadow`.
fn css_shadows(layers: Vec<BoxShadow>, _scale: f32) -> Vec<BoxShadow> {
    crate::primitives::shadows(layers)
}

fn icon_path(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/change-summary/clean/{name}.svg").into(),
        Look::Classic => format!("icons/change-summary/{name}.svg").into(),
    }
}

/// `ui-cs-in`: rows fade in from 4px below, 520ms, staggered by 70ms.
const ROW_IN: MotionSpec = MotionSpec::new(520, EASE_OUT_QUINT);

#[derive(IntoElement)]
pub struct ChangeSummary {
    id: ElementId,
    title: SharedString,
    period: Option<SharedString>,
    icon: Option<SharedString>,
    changes: Vec<MetricChange>,
    look: Option<Look>,
}

impl ChangeSummary {
    pub fn new(id: impl Into<ElementId>, changes: Vec<MetricChange>) -> Self {
        Self {
            id: id.into(),
            title: "What's changed".into(),
            period: None,
            icon: None,
            changes,
            look: None,
        }
    }

    /// The heading, "What's changed" by default.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Small text on the right of the heading, e.g. "This week".
    pub fn period(mut self, period: impl Into<SharedString>) -> Self {
        self.period = Some(period.into());
        self
    }

    /// An SVG under `assets/` to replace the heading icon, tinted like it.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// Overrides the global Clean/Classic look.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for ChangeSummary {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let mut p = Palette::get(look, cx);
        p.card_shadow = css_shadows(p.card_shadow, window.scale_factor());
        let m = Metrics::get(look);
        // Chrome paints the Clean 0.5px rules as one device pixel; GPUI
        // clips a literal 0.5px side away, so draw exactly one device pixel.
        let rule = match look {
            Look::Clean => px(1. / window.scale_factor()),
            Look::Classic => m.rule,
        };
        let clean = look == Look::Clean;
        let key = self.id.to_string();
        let count = self.changes.len();

        let title = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .min_w_0()
            .whitespace_nowrap()
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.title)
            .child(
                svg()
                    .path(
                        self.icon
                            .clone()
                            .unwrap_or_else(|| icon_path(look, "changes")),
                    )
                    .size(px(16.))
                    .flex_none()
                    .text_color(p.title_icon),
            )
            .child(self.title.clone());
        let head = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(title)
            .when_some(self.period.clone(), |d, period| {
                d.child(
                    div()
                        .flex_none()
                        .text_color(p.period)
                        .when(clean, |d| d.text_size(px(12.)).line_height(px(16.)))
                        .when(!clean, |d| d.text_size(px(13.)))
                        .child(period),
                )
            });
        let head = if clean {
            head.h(px(44.)).px(px(16.))
        } else {
            head.pt(px(11.))
                .px(px(18.))
                .pb(px(10.))
                .text_size(px(14.))
                .line_height(px(20.))
        };

        let rows = self.changes.iter().enumerate().map(|(ix, change)| {
            let (wash, ink) = p.tone(change.tone);
            let mark = match (change.tone, change.arrow()) {
                (Tone::Neutral, _) => "dash",
                (_, Direction::Up) => "arrow-up",
                (_, Direction::Down) => "arrow-down",
            };
            let last = ix + 1 == count;
            // Clean puts a hairline over every row (the heading is the first
            // row of the card); Classic only between rows.
            let ruled = clean || ix > 0;
            let mut label = div()
                .min_w_0()
                .flex_basis(relative(0.))
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.label)
                .child(change.label.clone());
            label.style().flex_grow = Some(0.8);
            let mut trend = div()
                .flex()
                .items_center()
                .gap(px(8.))
                .min_w_0()
                .flex_basis(relative(0.))
                .text_color(p.change)
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .rounded(m.trend_radius)
                        .bg(wash)
                        .child(
                            svg()
                                .path(icon_path(look, mark))
                                .size(px(12.))
                                .text_color(ink),
                        ),
                )
                .child(div().min_w_0().truncate().child(change.change.clone()));
            trend.style().flex_grow = Some(1.2);

            let hover_key = format!("{key}-row-{ix}");
            let row = div()
                .id(ElementId::NamedInteger("row".into(), ix as u64))
                .flex()
                .items_center()
                .gap(px(16.))
                .min_h(m.row_height)
                .px(m.row_pad)
                .when(ruled, |d| d.border_t(rule).border_color(p.line))
                // The card clips its corners; GPUI clips rectangles, so the
                // tint follows the corner itself.
                .when(last, |d| d.rounded_b(m.inner_radius))
                .when(!clean && ix == 0, |d| d.rounded_t(m.inner_radius))
                .hover_fill(hover_key, p.row_hover.opacity(0.), p.row_hover)
                .child(label)
                .child(trend);
            motion::rise_in(
                ElementId::NamedInteger(format!("{key}-in").into(), ix as u64),
                ROW_IN.with_delay(120 + ix as u64 * 70),
                4.,
                row,
            )
        });

        let list = div()
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_size(m.text_size)
            .line_height(px(20.))
            .children(rows);

        let root = div()
            .id(self.id.clone())
            .w_full()
            .max_w(px(480.))
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(p.label);
        match look {
            Look::Clean => root
                .overflow_hidden()
                .rounded(px(10.))
                .bg(p.card)
                .shadow(p.card_shadow)
                .child(head)
                .child(list),
            Look::Classic => root
                .p(px(6.))
                .rounded(px(22.))
                .when_some(p.tray, |d, tray| d.bg(tray))
                .child(head)
                .child(
                    list.rounded(px(16.))
                        .bg(p.card)
                        .shadow(p.card_shadow)
                        .when_some(p.card_border, |d, border| d.border_1().border_color(border)),
                ),
        }
    }
}

/// The site demo's four metrics.
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
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .w(px(480.))
            .child(ChangeSummary::new("change-summary", demo_changes()).period("This week"))
    })
}
