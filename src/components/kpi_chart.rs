//! KPI Chart – a headline total over a period, split by series, over a
//! smoothed trend (Pink: `shell/usage.rs` `render_usage_page`, the Usage
//! page's view card and summary card).
//!
//! A "Period" card switches the range with a segmented control. The summary
//! card below leads with the range's total in Display type, lists one row
//! per series (identity tile, series dot, share and amount), and ends on the
//! trend: monotone cubic areas and lines on a three-line grid with a value
//! gutter. Pointing at a period draws a guide, reads the period out in the
//! row's description and trailing caption, and shows a tooltip listing every
//! series.
//!
//! ```ignore
//! KpiChart::new("revenue", "Gross revenue")
//!     .range(KpiRange::new("30 days", dates).series(KpiSeries::new("Web", web)))
//!     .on_range_change(|ix, _, _| println!("range {ix}"))
//! ```

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyView, App, Context, ElementId, FontWeight, Hsla, PathBuilder, Render, SharedString, Window,
    canvas, div, point, prelude::*, px,
};

use crate::{
    pink::{self, ControlSize, FillTier, Pink, Segment, TextStyle, TypeRole, TypeStyleExt, layout},
    story,
};

pub const NAME: &str = "KPI Chart";
pub const DESCRIPTION: &str = "A period's headline total split into one row per series, over smoothed trend lines – point at a period for a guide, a readout and every series' value";

/// The plot height, value-axis gutter and gutter gap (Pink `usage.rs`
/// `CHART_HEIGHT` / `CHART_AXIS_WIDTH` / `CHART_AXIS_GAP`).
const CHART_HEIGHT: f32 = 200.0;
const CHART_AXIS_WIDTH: f32 = 56.0;
const CHART_AXIS_GAP: f32 = 8.0;
/// The settings page column the Usage page sits in: 720 wide, `px 32`
/// (Pink `settings/widgets.rs` `page_column`).
const CONTENT_WIDTH: f32 = layout::PAGE_MAX_WIDTH - 2.0 * 32.0;
/// Period control width per segment (Pink: four periods in 300).
const SEGMENT_WIDTH: f32 = 75.0;

type RangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type Formatter = Rc<dyn Fn(f32) -> String>;

/// One series of the chart, with its row in the summary.
#[derive(Clone)]
pub struct KpiSeries {
    pub name: SharedString,
    pub values: Vec<f32>,
    /// Overrides the series color (Pink's provider tones by default).
    pub color: Option<Hsla>,
    /// An icon asset for the row's identity tile.
    pub icon: Option<SharedString>,
    /// Leads the row's description, e.g. "1,204 orders".
    pub detail: Option<SharedString>,
}

impl KpiSeries {
    pub fn new(name: impl Into<SharedString>, values: Vec<f32>) -> Self {
        Self {
            name: name.into(),
            values,
            color: None,
            icon: None,
            detail: None,
        }
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// A selectable period: its points' labels and the series over them.
/// Every range should list the same series in the same order.
#[derive(Clone)]
pub struct KpiRange {
    /// The segment label, e.g. "30 days".
    pub label: SharedString,
    /// One short label per point for the axis and readout, e.g. "Oct 12".
    pub points: Vec<SharedString>,
    /// One long label per point for the hover detail, e.g. "Mon, Oct 12"
    /// (falls back to `points`).
    pub point_details: Vec<SharedString>,
    pub series: Vec<KpiSeries>,
    /// The Period row's description, e.g. "One point per day."
    pub caption: Option<SharedString>,
    /// The chart row's title, e.g. "Daily revenue".
    pub trend_title: Option<SharedString>,
    /// The chart row's description while nothing is hovered.
    pub trend_hint: Option<SharedString>,
}

impl KpiRange {
    pub fn new(label: impl Into<SharedString>, points: Vec<SharedString>) -> Self {
        Self {
            label: label.into(),
            points,
            point_details: Vec::new(),
            series: Vec::new(),
            caption: None,
            trend_title: None,
            trend_hint: None,
        }
    }

    pub fn details(mut self, details: Vec<SharedString>) -> Self {
        self.point_details = details;
        self
    }

    pub fn series(mut self, series: KpiSeries) -> Self {
        self.series.push(series);
        self
    }

    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// The chart row's title and its resting description.
    pub fn trend(mut self, title: impl Into<SharedString>, hint: impl Into<SharedString>) -> Self {
        self.trend_title = Some(title.into());
        self.trend_hint = Some(hint.into());
        self
    }

    fn short(&self, ix: usize) -> SharedString {
        self.points.get(ix).cloned().unwrap_or_default()
    }

    fn long(&self, ix: usize) -> SharedString {
        self.point_details
            .get(ix)
            .cloned()
            .unwrap_or_else(|| self.short(ix))
    }

    fn value(&self, series: usize, ix: usize) -> f32 {
        self.series
            .get(series)
            .and_then(|s| s.values.get(ix))
            .copied()
            .unwrap_or(0.)
    }

    /// The sum of every series at point `ix`.
    fn total_at(&self, ix: usize) -> f32 {
        (0..self.series.len())
            .map(|s| self.value(s, ix) as f64)
            .sum::<f64>() as f32
    }
}

#[derive(IntoElement)]
pub struct KpiChart {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    ranges: Vec<KpiRange>,
    default_range: usize,
    format: Formatter,
    axis_format: Formatter,
    on_range_change: Option<RangeHandler>,
}

impl KpiChart {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            ranges: Vec::new(),
            default_range: 0,
            format: Rc::new(grouped),
            axis_format: Rc::new(compact),
            on_range_change: None,
        }
    }

    /// The summary card's caption under its title, e.g. "Before refunds and tax".
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn range(mut self, range: KpiRange) -> Self {
        self.ranges.push(range);
        self
    }

    /// The range shown first (an index into the added ranges).
    pub fn default_range(mut self, ix: usize) -> Self {
        self.default_range = ix;
        self
    }

    /// Formats the headline, row amounts and readouts (default: grouped
    /// integers, "48,210").
    pub fn format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Formats the value-axis labels (default: compact, "48.21K").
    pub fn axis_format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.axis_format = Rc::new(format);
        self
    }

    pub fn on_range_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_range_change = Some(Rc::new(handler));
        self
    }
}

struct State {
    range: usize,
    /// The period under the pointer (Pink `usage_hover_period_start`).
    hover: Option<usize>,
}

/// Series colors: Pink's provider tones (`usage.rs` `provider_tone`: the
/// text color, then #5b9bbd and #d97757), then Pink's project tints.
fn series_tone(ix: usize, t: &Pink) -> Hsla {
    match ix {
        0 => t.text,
        1 => gpui::rgb(0x5b9bbd).into(),
        2 => gpui::rgb(0xd97757).into(),
        _ => pink::project_tint(ix - 3),
    }
}

impl RenderOnce for KpiChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let default_range = self.default_range;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State {
                range: default_range,
                hover: None,
            },
        );
        if self.ranges.is_empty() {
            return div().id(self.id.clone());
        }
        let (range_ix, hover) = {
            let s = state.read(cx);
            (s.range.min(self.ranges.len() - 1), s.hover)
        };
        let range = &self.ranges[range_ix];
        let n = range.points.len();
        let hover = hover.filter(|h| *h < n);
        let format = self.format.clone();
        let tones: Vec<Hsla> = range
            .series
            .iter()
            .enumerate()
            .map(|(ix, s)| s.color.unwrap_or_else(|| series_tone(ix, &t)))
            .collect();
        let sums: Vec<f64> = range
            .series
            .iter()
            .map(|s| s.values.iter().map(|v| *v as f64).sum())
            .collect();
        let total: f64 = sums.iter().sum();

        // ---- the view card: the period switch (Pink `view_card`) ----
        let period_card = (self.ranges.len() > 1).then(|| {
            let segments = self
                .ranges
                .iter()
                .enumerate()
                .map(|(ix, r)| Segment::new(ix.to_string(), r.label.clone()))
                .collect();
            let state = state.clone();
            let handler = self.on_range_change.clone();
            pink::section_card(&t).mt_0().child(
                pink::card_row(true, &t)
                    .child(pink::row_text("Period", range.caption.clone(), &t))
                    .child(pink::row_controls().child(pink::segmented(
                        format!("{key}-period"),
                        segments,
                        range_ix,
                        SEGMENT_WIDTH * self.ranges.len() as f32,
                        ControlSize::Md,
                        &t,
                        move |ix: &usize, window, cx| {
                            let ix = *ix;
                            state.update(cx, |s, cx| {
                                if s.range != ix {
                                    s.range = ix;
                                    s.hover = None;
                                    cx.notify();
                                }
                            });
                            if let Some(f) = &handler {
                                f(ix, window, cx);
                            }
                        },
                    ))),
            )
        });

        // ---- the summary card: total, one row per series, then the chart ----
        let mut summary = pink::section_card(&t)
            .when(period_card.is_none(), |card| card.mt_0())
            .child(pink::section_header(
                self.title.clone(),
                self.subtitle.clone(),
                &t,
            ))
            .child(
                pink::card_row(false, &t).child(
                    div()
                        .min_w_0()
                        .type_style(TextStyle::Display)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(format(total as f32)),
                ),
            );
        for (ix, series) in range.series.iter().enumerate() {
            if sums[ix] == 0. {
                continue;
            }
            let share = if total == 0. { 0. } else { sums[ix] / total };
            let description = match &series.detail {
                Some(detail) => format!("{detail} · {} of total", percent(share)),
                None => format!("{} of total", percent(share)),
            };
            summary = summary.child(
                pink::card_row(false, &t)
                    .when_some(series.icon.clone(), |row, icon| {
                        row.child(series_tile(icon, &t))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(layout::ROW_TEXT_MIN))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                // The dot is the series' color in the chart below.
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .flex_none()
                                            .size(px(8.0))
                                            .rounded_full()
                                            .bg(tones[ix]),
                                    )
                                    .child(pink::row_title(series.name.clone(), &t)),
                            )
                            .child(pink::row_description(description, &t)),
                    )
                    .child(
                        pink::row_controls().child(
                            div()
                                .type_role(TypeRole::Chrome)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(format(sums[ix] as f32)),
                        ),
                    ),
            );
        }

        // The chart row: its title, the hovered period's detail (or the
        // resting hint), the trailing readout, then the chart itself.
        let hover_detail = hover.map(|h| {
            let parts: Vec<String> = range
                .series
                .iter()
                .enumerate()
                .filter(|(ix, _)| range.value(*ix, h) > 0.)
                .map(|(ix, s)| format!("{} {}", s.name, format(range.value(ix, h))))
                .collect();
            if parts.is_empty() {
                format!("{} · No activity", range.long(h))
            } else {
                format!(
                    "{} · {} · Total {}",
                    range.long(h),
                    parts.join(" · "),
                    format(range.total_at(h))
                )
            }
        });
        let hover_readout =
            hover.map(|h| format!("{} · Total {}", range.short(h), format(range.total_at(h))));
        let description: SharedString = match hover_detail {
            Some(detail) => detail.into(),
            None => range
                .trend_hint
                .clone()
                .unwrap_or_else(|| "Hover a point for each series' value.".into()),
        };
        summary = summary.child(
            pink::card_row(false, &t)
                .child(pink::row_text(
                    range.trend_title.clone().unwrap_or_else(|| "Trend".into()),
                    Some(description),
                    &t,
                ))
                .when_some(hover_readout, |row, readout| {
                    row.child(
                        pink::row_controls().child(
                            div()
                                .text_size(px(11.0))
                                .line_height(px(14.0))
                                .text_color(t.readable_muted(0.85))
                                .child(readout),
                        ),
                    )
                })
                .child(pink::row_block().child(trend_chart(
                    &key,
                    range,
                    &tones,
                    &sums,
                    hover,
                    &self.format,
                    &self.axis_format,
                    &state,
                    &t,
                ))),
        );

        div()
            .id(self.id.clone())
            .w_full()
            .max_w(px(CONTENT_WIDTH))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .children(period_card)
            .child(summary)
    }
}

/// A series' identity tile (Pink `usage.rs` `provider_tile`): the row tile
/// frame, 28 square, radius 7, 1 px `border`, `fill(Quaternary)`, with the
/// mark at full `text` strength so it reads as identity, not decoration.
fn series_tile(icon: SharedString, t: &Pink) -> gpui::Div {
    div()
        .flex_none()
        .size(px(28.0))
        .rounded(px(pink::radius::TILE))
        .border_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(icon).size(px(16.0)).text_color(t.text))
}

/// The trend (Pink `usage.rs` `usage_chart`): a nice-number scale, three
/// gridlines, a flat 12% area and a 2 px line per series (largest painted
/// first), a 1 px guide on the hovered period, one hover cell per period
/// with a tooltip, and 11/14 axis labels in `readable_muted(0.85)`.
#[allow(clippy::too_many_arguments)]
fn trend_chart(
    key: &str,
    range: &KpiRange,
    tones: &[Hsla],
    sums: &[f64],
    hover: Option<usize>,
    format: &Formatter,
    axis_format: &Formatter,
    state: &gpui::Entity<State>,
    t: &Pink,
) -> gpui::Div {
    let n = range.points.len();
    let peak = range
        .series
        .iter()
        .flat_map(|s| s.values.iter().copied())
        .fold(0f32, f32::max);
    let scale = nice_scale(peak);
    let labels: [String; 3] = if peak <= 0. {
        [String::new(), String::new(), "0".into()]
    } else {
        [scale, scale / 2., 0.].map(|v| if v == 0. { "0".into() } else { axis_format(v) })
    };

    // Each series from zero; the largest is painted first so smaller
    // series and their lines stay visible.
    let mut order: Vec<usize> = (0..range.series.len()).collect();
    order.sort_by(|a, b| sums[*b].total_cmp(&sums[*a]));
    let lines: Vec<(Hsla, Vec<f32>)> = order
        .into_iter()
        .filter(|ix| range.series[*ix].values.iter().any(|v| *v > 0.))
        .map(|ix| {
            let values = (0..n).map(|i| range.value(ix, i)).collect();
            (tones[ix], values)
        })
        .collect();
    let grid = t.border;
    let guide = t.text_muted;
    let plot = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            if width < 2. || height < 2. {
                return;
            }
            let (x0, y0) = (bounds.origin.x, bounds.origin.y);
            for ratio in [0.0_f32, 0.5, 1.0] {
                let mut path = PathBuilder::stroke(px(1.0));
                let y = y0 + px(ratio * height);
                path.move_to(point(x0, y));
                path.line_to(point(x0 + px(width), y));
                if let Ok(path) = path.build() {
                    window.paint_path(path, grid);
                }
            }
            if n == 0 {
                return;
            }
            let x_of = |i: usize| {
                if n == 1 {
                    width / 2.
                } else {
                    width * i as f32 / (n - 1) as f32
                }
            };
            for fill in [true, false] {
                for (color, values) in &lines {
                    let coords: Vec<(f32, f32)> = values
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            let y = height - (v / scale).clamp(0., 1.) * (height - 8.);
                            (x_of(i), y)
                        })
                        .collect();
                    let mut path = if fill {
                        PathBuilder::fill()
                    } else {
                        PathBuilder::stroke(px(2.0))
                    };
                    path.move_to(point(x0 + px(coords[0].0), y0 + px(coords[0].1)));
                    // A monotone cubic never draws a false peak between two
                    // measured periods.
                    let tangents = monotone_tangents(&coords);
                    for (i, pair) in coords.windows(2).enumerate() {
                        let ((xa, ya), (xb, yb)) = (pair[0], pair[1]);
                        let dx = (xb - xa) / 3.;
                        path.cubic_bezier_to(
                            point(x0 + px(xb), y0 + px(yb)),
                            point(x0 + px(xa + dx), y0 + px(ya + tangents[i] * dx)),
                            point(x0 + px(xb - dx), y0 + px(yb - tangents[i + 1] * dx)),
                        );
                    }
                    if fill {
                        let last = coords[coords.len() - 1].0;
                        path.line_to(point(x0 + px(last), y0 + px(height)));
                        path.line_to(point(x0 + px(coords[0].0), y0 + px(height)));
                        path.close();
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, if fill { color.opacity(0.12) } else { *color });
                    }
                }
            }
            if let Some(h) = hover {
                let x = x_of(h);
                let mut line = PathBuilder::stroke(px(1.0));
                line.move_to(point(x0 + px(x), y0));
                line.line_to(point(x0 + px(x), y0 + px(height)));
                if let Ok(line) = line.build() {
                    window.paint_path(line, guide);
                }
            }
        },
    )
    .w_full()
    .h(px(CHART_HEIGHT));

    // One hover cell per period, each with the period's tooltip.
    let cells = (0..n).map(|i| {
        let mut detail = range.long(i).to_string();
        for (ix, s) in range.series.iter().enumerate() {
            detail.push_str(&format!("\n{} {}", s.name, format(range.value(ix, i))));
        }
        detail.push_str(&format!("\nTotal {}", format(range.total_at(i))));
        let detail = SharedString::from(detail);
        let state = state.clone();
        div()
            .id(SharedString::from(format!("{key}-period-{i}")))
            .flex_1()
            .h_full()
            .on_hover(move |hovered, _, cx| {
                state.update(cx, |s, cx| {
                    if *hovered {
                        s.hover = Some(i);
                    } else if s.hover == Some(i) {
                        s.hover = None;
                    }
                    cx.notify();
                })
            })
            .tooltip(move |_, cx| {
                cx.new(|_| PeriodTooltip {
                    text: detail.clone(),
                })
                .into()
            })
    });

    let x_labels: [SharedString; 3] = if n == 0 {
        Default::default()
    } else {
        [0, n / 2, n - 1].map(|i| range.short(i).to_uppercase().into())
    };
    let label_tone = t.readable_muted(0.85);
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .w_full()
                .flex()
                .gap(px(CHART_AXIS_GAP))
                .child(
                    div()
                        .w(px(CHART_AXIS_WIDTH))
                        .h(px(CHART_HEIGHT))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .text_align(gpui::TextAlign::Right)
                        .text_size(px(11.0))
                        .line_height(px(14.0))
                        .text_color(label_tone)
                        .children(labels),
                )
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h(px(CHART_HEIGHT))
                        .child(plot)
                        .child(div().absolute().inset_0().flex().children(cells)),
                ),
        )
        .child(
            div()
                .w_full()
                .pl(px(CHART_AXIS_WIDTH + CHART_AXIS_GAP))
                .flex()
                .justify_between()
                .text_size(px(11.0))
                .line_height(px(14.0))
                .text_color(label_tone)
                .children(x_labels),
        )
}

/// A period's hover tooltip (Pink `shell.rs` `SurfaceTabTooltip`): the
/// tooltip card, at most 380 wide, `px 8 / py 5`, 11/14 `text`, one line per
/// series.
struct PeriodTooltip {
    text: SharedString,
}

impl Render for PeriodTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        pink::tooltip_card(&t)
            .max_w(px(380.0))
            .px(px(8.0))
            .py(px(5.0))
            .text_size(px(11.0))
            .line_height(px(14.0))
            .text_color(t.text)
            .child(self.text.clone())
    }
}

/// The value axis' top: a 1-2-5 unit near half the peak, rounded up to a
/// whole number of units (Pink `usage_chart`).
fn nice_scale(peak: f32) -> f32 {
    if peak <= 0. {
        return 1.;
    }
    let rough = peak as f64 / 2.;
    let magnitude = 10_f64.powf(rough.log10().floor());
    let unit = [1., 2., 5., 10.]
        .into_iter()
        .find(|step| step * magnitude >= rough)
        .unwrap_or(10.)
        * magnitude;
    ((peak as f64 / unit).ceil() * unit) as f32
}

/// Fritsch–Carlson tangents over `(x, y)` points (Pink
/// `monotone_usage_tangents`): a smooth curve that never overshoots the data.
fn monotone_tangents(points: &[(f32, f32)]) -> Vec<f32> {
    if points.len() < 2 {
        return vec![0.; points.len()];
    }
    let slopes: Vec<f32> = points
        .windows(2)
        .map(|pair| {
            let dx = pair[1].0 - pair[0].0;
            if dx == 0. {
                0.
            } else {
                (pair[1].1 - pair[0].1) / dx
            }
        })
        .collect();
    let mut tangents = vec![0.; points.len()];
    tangents[0] = slopes[0];
    tangents[points.len() - 1] = slopes[slopes.len() - 1];
    for i in 1..points.len() - 1 {
        let (previous, next) = (slopes[i - 1], slopes[i]);
        tangents[i] = if previous * next <= 0. {
            0.
        } else {
            (previous + next) / 2.
        };
    }
    for (i, slope) in slopes.iter().copied().enumerate() {
        if slope == 0. {
            tangents[i] = 0.;
            tangents[i + 1] = 0.;
            continue;
        }
        let a = tangents[i] / slope;
        let b = tangents[i + 1] / slope;
        let magnitude = a * a + b * b;
        if magnitude > 9. {
            let scale = 3. / magnitude.sqrt();
            tangents[i] = scale * a * slope;
            tangents[i + 1] = scale * b * slope;
        }
    }
    tangents
}

/// A share as Pink writes it (`usage_percent`): "38.4%", "<0.1%".
fn percent(share: f64) -> String {
    let percent = share * 100.;
    if percent > 0. && percent < 0.1 {
        "<0.1%".into()
    } else {
        format!("{percent:.1}%")
    }
}

/// 48210 → "48,210" (Pink `count`).
fn grouped(v: f32) -> String {
    let n = v.round() as i64;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// 48210 → "48.21K", 125000 → "125K" (Pink `compact_tokens`).
fn compact(v: f32) -> String {
    let value = v.max(0.).round() as u64;
    for (threshold, suffix) in [
        (1_000_000_000_000_u64, "T"),
        (1_000_000_000, "B"),
        (1_000_000, "M"),
        (1_000, "K"),
    ] {
        if value >= threshold {
            let scaled = value as f64 / threshold as f64;
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
    grouped(value as f32)
}

/// 1234.5 → "$1,234.50" (Pink `money`).
fn money(v: f32) -> String {
    let cents = (v.max(0.) as f64 * 100.).round() as u64;
    format!("${}.{:02}", grouped((cents / 100) as f32), cents % 100)
}

// ---------------------------------------------------------------------------
// Demo data: a shop's gross revenue by channel, the last 24 hours and the
// last 90 days to Thu Oct 22, 2026.
// ---------------------------------------------------------------------------

/// A repeatable 0..1 jitter.
fn jitter(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x as f32 / u32::MAX as f32
}

/// ("Oct", 22) for a day of the year 2026.
fn month_day(day_of_year: usize) -> (&'static str, usize) {
    const MONTHS: [(&str, usize); 12] = [
        ("Jan", 31),
        ("Feb", 28),
        ("Mar", 31),
        ("Apr", 30),
        ("May", 31),
        ("Jun", 30),
        ("Jul", 31),
        ("Aug", 31),
        ("Sep", 30),
        ("Oct", 31),
        ("Nov", 30),
        ("Dec", 31),
    ];
    let mut d = day_of_year;
    for (name, len) in MONTHS {
        if d <= len {
            return (name, d);
        }
        d -= len;
    }
    ("Dec", 31)
}

/// "9 AM" for an hour of the day.
fn hour_label(hour: usize) -> String {
    let h = hour % 12;
    format!(
        "{} {}",
        if h == 0 { 12 } else { h },
        if hour < 12 { "AM" } else { "PM" }
    )
}

fn hour_minutes(hour: usize) -> String {
    let h = hour % 12;
    format!(
        "{}:00 {}",
        if h == 0 { 12 } else { h },
        if hour % 24 < 12 { "AM" } else { "PM" }
    )
}

/// (name, icon, daily revenue at the start and end of the 90 days, weekend
/// factor, average order value).
const CHANNELS: [(&str, &str, f32, f32, f32, f32); 3] = [
    (
        "Web",
        "icons/kpi-chart/globe.svg",
        6_100.,
        8_000.,
        0.88,
        64.,
    ),
    (
        "iOS app",
        "icons/kpi-chart/smartphone.svg",
        3_200.,
        4_600.,
        1.08,
        41.,
    ),
    (
        "Desktop app",
        "icons/kpi-chart/laptop.svg",
        1_250.,
        1_600.,
        0.8,
        118.,
    ),
];

fn demo_range(
    label: &str,
    points: Vec<SharedString>,
    details: Vec<SharedString>,
    values: [Vec<f32>; 3],
    hourly: bool,
) -> KpiRange {
    let range = KpiRange::new(label.to_string(), points)
        .details(details)
        .caption(if hourly {
            "One point per hour."
        } else {
            "One point per day."
        })
        .trend(
            if hourly {
                "Hourly revenue"
            } else {
                "Daily revenue"
            },
            if hourly {
                "Each point covers one hour. Hover for channel values."
            } else {
                "Each point covers one day. Hover for channel values."
            },
        );
    values.into_iter().zip(CHANNELS).fold(
        range,
        |range, (values, (name, icon, .., order_value))| {
            let sum: f32 = values.iter().sum();
            let orders = grouped((sum / order_value).round());
            range.series(
                KpiSeries::new(name, values)
                    .icon(icon)
                    .detail(format!("{orders} orders")),
            )
        },
    )
}

fn demo_chart() -> KpiChart {
    const DAYS: usize = 90;
    // Oct 22, 2026 is day 295 and a Thursday.
    const LAST_DAY: usize = 295;
    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let day_of = |d: usize| LAST_DAY - (DAYS - 1 - d);
    let weekday = |d: usize| (3 + day_of(d) as isize - LAST_DAY as isize).rem_euclid(7) as usize;
    let short: Vec<SharedString> = (0..DAYS)
        .map(|d| {
            let (month, day) = month_day(day_of(d));
            format!("{month} {day}").into()
        })
        .collect();
    let long: Vec<SharedString> = (0..DAYS)
        .map(|d| {
            let (month, day) = month_day(day_of(d));
            format!("{}, {month} {day}", WEEKDAYS[weekday(d)]).into()
        })
        .collect();
    let daily: Vec<Vec<f32>> = CHANNELS
        .iter()
        .enumerate()
        .map(|(c, (_, _, start, end, weekend, _))| {
            (0..DAYS)
                .map(|d| {
                    let f = d as f32 / (DAYS - 1) as f32;
                    // Growth, a late-summer lull and a sale in early October.
                    let trend = start + (end - start) * (f * f * 0.4 + f * 0.6);
                    let lull = 1. - 0.07 * (-(f - 0.3).powi(2) / 0.008).exp();
                    let sale = 1. + 0.12 * (-(f - 0.8).powi(2) / 0.0015).exp();
                    let week = if weekday(d) >= 5 { *weekend } else { 1. };
                    let noise = 1. + (jitter((c * 1000 + d) as u32) - 0.5) * 0.16;
                    (trend * lull * sale * week * noise * 100.).round() / 100.
                })
                .collect()
        })
        .collect();
    let days = |label: &str, count: usize| {
        let from = DAYS - count;
        demo_range(
            label,
            short[from..].to_vec(),
            long[from..].to_vec(),
            [0, 1, 2].map(|c| daily[c][from..].to_vec()),
            false,
        )
    };

    // The past 24 hours: 3 PM Wednesday to 2 PM Thursday.
    let hours: Vec<usize> = (0..24).map(|h| 15 + h).collect();
    let hour_points: Vec<SharedString> = hours.iter().map(|h| hour_label(h % 24).into()).collect();
    let hour_details: Vec<SharedString> = hours
        .iter()
        .map(|h| {
            let day = if *h < 24 { 21 } else { 22 };
            format!(
                "Oct {day}, {}–{}",
                hour_minutes(h % 24),
                hour_minutes(h + 1)
            )
            .into()
        })
        .collect();
    // Share of a day's orders by hour: quiet nights, a lunch bump and an
    // evening peak.
    let curve = |hour: usize| {
        let h = hour as f32;
        0.15 + 0.9 * (-(h - 12.5).powi(2) / 8.).exp() + 1.3 * (-(h - 20.5).powi(2) / 6.).exp()
    };
    let norm: f32 = (0..24).map(curve).sum();
    let hourly = [0, 1, 2].map(|c| {
        let (_, _, _, end, _, _) = CHANNELS[c];
        hours
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let noise = 1. + (jitter((c * 7919 + i) as u32 + 50_000) - 0.5) * 0.3;
                (end * curve(h % 24) / norm * noise * 100.).round() / 100.
            })
            .collect::<Vec<f32>>()
    });

    KpiChart::new("kpi-chart", "Gross revenue")
        .subtitle("Paid orders across every channel · before refunds and tax")
        .format(money)
        .axis_format(|v| format!("${}", compact(v)))
        .range(demo_range(
            "Past 24h",
            hour_points,
            hour_details,
            hourly,
            true,
        ))
        .range(days("7 days", 7))
        .range(days("30 days", 30))
        .range(days("90 days", 90))
        .default_range(2)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| demo_chart())
}
