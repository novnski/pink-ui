//! Breakdown Chart – a small data cube shown the way pink's Usage page shows
//! spend: a view card that picks what to show, a summary card with the total
//! and a stacked-column chart, and a Breakdown card with the same numbers as
//! a table.
//!
//! pink: `shell/usage.rs` `render_usage_page`. The view card is its
//! "Show" / "Period" card (settings rows, each with a segmented control);
//! here the rows pick the measure, the dimension along the x axis and the one
//! stacked in each column (picking the other row's dimension swaps the two).
//! The summary card is pink's: the total in Display type, here with the
//! parts' series dots beside it (pink puts each dot on a provider row), then
//! the chart row. The chart reproduces `usage_chart`'s frame: a 56 px value gutter,
//! three hairline gridlines, caption labels, and per-column hover cells with
//! a tooltip and a readout in the row above. Pink has no stacked columns, so
//! each part is painted the way pink paints an area series: a 12% body under
//! a 2 px line in the series tone. The table is pink's Breakdown table
//! (`usage_table_row` anatomy) with its segmented toggle in the section
//! header; here it orders the columns and rows, which glide into place.
//!
//! ```ignore
//! BreakdownChart::new("spend", "Cloud spend", cube)
//!     .slice(0)   // dimension along the x axis
//!     .segment(2) // dimension stacked inside each column
//! ```

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyElement, AnyView, App, Bounds, Context, Corners, ElementId, FocusHandle, FontWeight, Hsla,
    KeyDownEvent, MouseButton, Render, SharedString, TextAlign, Window, canvas, div, fill, point,
    prelude::*, px, quad, relative, rgb, size,
};

use crate::{
    motion,
    pink::{self, ControlSize, FillTier, Pink, Segment, TextStyle, TypeStyleExt, layout, radius},
    story,
};

pub const NAME: &str = "Breakdown Chart";
pub const DESCRIPTION: &str = "A measure split two ways on pink's usage page – Show, Group by and Split by rows on segmented tracks, the total with its parts' series dots, stacked columns in the usage chart's frame with hover readouts, and a Breakdown table whose columns and rows glide into order";

/// pink `icons::CHART`, the Usage page's empty-state tile.
const ICON_CHART: &str = "icons/breakdown-chart/chart.svg";

/// pink `usage.rs` `CHART_HEIGHT`, `CHART_AXIS_WIDTH`, `CHART_AXIS_GAP`.
const CHART_HEIGHT: f32 = 200.0;
const CHART_AXIS_WIDTH: f32 = 56.0;
const CHART_AXIS_GAP: f32 = 8.0;
/// Headroom above the tallest mark (pink `usage_chart`: `height - 8`).
const CHART_HEADROOM: f32 = 8.0;
/// A column takes 40% of its slot, up to this width.
const COLUMN_MAX_WIDTH: f32 = 48.0;
/// A series' line on top of its body (pink `usage_chart`: 2 px stroke).
const SERIES_LINE: f32 = 2.0;
/// A series' body under its line (pink `usage_chart`: fill at 12%).
const SERIES_BODY_ALPHA: f32 = 0.12;
/// The hovered column's wash reaches this far past its sides.
const HOVER_WASH_PAD: f32 = 8.0;
/// pink `usage_table_row`: header and body row heights.
const TABLE_HEADER_HEIGHT: f32 = 30.0;
const TABLE_ROW_HEIGHT: f32 = 36.0;

/// How a measure prints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureFormat {
    /// "$12.4K" in the chart and table, "$184,213.52" as the total.
    Currency,
    /// "1.2M".
    Count,
}

#[derive(Clone)]
pub struct Measure {
    pub name: SharedString,
    pub format: MeasureFormat,
}

impl Measure {
    pub fn new(name: impl Into<SharedString>, format: MeasureFormat) -> Self {
        Self {
            name: name.into(),
            format,
        }
    }

    /// A figure in the chart, the readouts and the table (pink
    /// `compact_tokens`, with a "$" for currency), short enough for the
    /// value gutter and a table with a column per part.
    fn fmt(&self, v: f64) -> String {
        let body = compact(v);
        match self.format {
            MeasureFormat::Currency => format!("${body}"),
            MeasureFormat::Count => body,
        }
    }

    /// The summary card's total (pink `money` for cost, `compact_tokens`
    /// for counts).
    fn fmt_total(&self, v: f64) -> String {
        match self.format {
            MeasureFormat::Currency => money(v),
            MeasureFormat::Count => compact(v),
        }
    }
}

/// A categorical axis of the cube, e.g. Team with its five teams.
#[derive(Clone)]
pub struct Dimension {
    pub name: SharedString,
    pub values: Vec<SharedString>,
}

impl Dimension {
    pub fn new(name: impl Into<SharedString>, values: Vec<SharedString>) -> Self {
        Self {
            name: name.into(),
            values,
        }
    }
}

/// One cell of the cube: a value index per dimension and a number per measure.
#[derive(Clone)]
pub struct Fact {
    pub coords: Vec<usize>,
    pub values: Vec<f64>,
}

#[derive(Clone)]
pub struct Cube {
    pub dimensions: Vec<Dimension>,
    pub measures: Vec<Measure>,
    pub facts: Vec<Fact>,
}

impl Cube {
    /// `[slice value][segment value]` totals of `measure`.
    fn matrix(&self, measure: usize, slice: usize, segment: usize) -> Vec<Vec<f64>> {
        let rows = self.dimensions[slice].values.len();
        let cols = self.dimensions[segment].values.len();
        let mut m = vec![vec![0.; cols]; rows];
        for f in &self.facts {
            let (Some(&r), Some(&c), Some(&v)) = (
                f.coords.get(slice),
                f.coords.get(segment),
                f.values.get(measure),
            ) else {
                continue;
            };
            if r < rows && c < cols {
                m[r][c] += v;
            }
        }
        m
    }
}

#[derive(IntoElement)]
pub struct BreakdownChart {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    cube: Rc<Cube>,
    measure: usize,
    slice: usize,
    segment: usize,
    largest_first: bool,
}

impl BreakdownChart {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, cube: Cube) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            cube: Rc::new(cube),
            measure: 0,
            slice: 0,
            segment: 1,
            largest_first: false,
        }
    }

    /// The caption under the summary card's title.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The measure shown first.
    pub fn measure(mut self, ix: usize) -> Self {
        self.measure = ix;
        self
    }

    /// The dimension laid along the x axis at first.
    pub fn slice(mut self, ix: usize) -> Self {
        self.slice = ix;
        self
    }

    /// The dimension stacked inside each column at first.
    pub fn segment(mut self, ix: usize) -> Self {
        self.segment = ix;
        self
    }

    /// Start with the columns and rows ordered largest first.
    pub fn largest_first(mut self, on: bool) -> Self {
        self.largest_first = on;
        self
    }
}

struct State {
    measure: usize,
    slice: usize,
    segment: usize,
    largest_first: bool,
    /// Slice value under the pointer (in the chart or the table) or picked
    /// with the arrow keys.
    hot: Option<usize>,
    focus: FocusHandle,
}

impl State {
    /// Puts dimension `ix` along the x axis; the split's dimension swaps in.
    fn choose_slice(&mut self, ix: usize) {
        if ix == self.segment {
            self.segment = self.slice;
        }
        self.slice = ix;
        self.hot = None;
    }

    /// Stacks dimension `ix` in the columns; the x axis' dimension swaps in.
    fn choose_segment(&mut self, ix: usize) {
        if ix == self.slice {
            self.slice = self.segment;
        }
        self.segment = ix;
        self.hot = None;
    }
}

/// The parts' series tones. The first three are pink's Usage chart series
/// (`usage.rs` `provider_tone`: `text`, `#5b9bbd`, `#d97757`); further parts
/// take pink's project hues (`sidebar_projects.rs` `PROJECT_HUES`) that sit
/// clear of those, floored to the non-text contrast target.
fn series_tones(t: &Pink, count: usize) -> Vec<Hsla> {
    let mut tones = vec![t.text, rgb(0x5b9bbd).into(), rgb(0xd97757).into()];
    // Hues 150, 40, 250 and 325 of the project ladder.
    for hue_ix in [1, 2, 0, 4] {
        tones.push(t.readable_icon(pink::project_tint(hue_ix)));
    }
    (0..count).map(|ix| tones[ix % tones.len()]).collect()
}

/// pink `usage.rs` widths for its segmented rows: 150 for two options, 216
/// for three, 300 for four; about 75 per option past that.
fn segmented_width(count: usize) -> f32 {
    match count {
        0..=2 => 150.0,
        3 => 216.0,
        4 => 300.0,
        n => 75.0 * n as f32,
    }
}

/// The chart's top value: twice a round half-step at or above half the
/// peak, so the three gridlines (0, ½, 1) land on round figures (pink
/// `usage_chart` scales the same way, from a coarser 1-2-5 ladder).
fn chart_scale(peak: f64) -> f64 {
    if peak <= 0.0 {
        return 1.0;
    }
    let half = peak / 2.0;
    let magnitude = 10_f64.powf(half.log10().floor());
    let step = [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0]
        .into_iter()
        .find(|step| step * magnitude >= half - 1e-9)
        .unwrap_or(10.0)
        * magnitude;
    step * 2.0
}

impl RenderOnce for BreakdownChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let cube = self.cube.clone();
        let dims = cube.dimensions.len();
        if dims < 2 || cube.measures.is_empty() {
            return empty_card(&t).id(self.id.clone()).into_any_element();
        }
        let (m0, s0, g0, sorted0) = (
            self.measure.min(cube.measures.len() - 1),
            self.slice.min(dims - 1),
            self.segment.min(dims - 1),
            self.largest_first,
        );
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| State {
                measure: m0,
                slice: s0,
                segment: if g0 == s0 { (s0 + 1) % dims } else { g0 },
                largest_first: sorted0,
                hot: None,
                focus: cx.focus_handle().tab_stop(true),
            },
        );
        let s = state.read(cx);
        let measure_ix = s.measure.min(cube.measures.len() - 1);
        let (slice_dim, seg_dim) = (s.slice, s.segment);
        let (largest_first, hot) = (s.largest_first, s.hot);
        let focus = s.focus.clone();

        let measure = cube.measures[measure_ix].clone();
        let slice_name = cube.dimensions[slice_dim].name.clone();
        let seg_name = cube.dimensions[seg_dim].name.clone();
        let slices = cube.dimensions[slice_dim].values.clone();
        let parts = cube.dimensions[seg_dim].values.clone();
        let matrix = cube.matrix(measure_ix, slice_dim, seg_dim);
        let totals: Vec<f64> = matrix.iter().map(|r| r.iter().sum()).collect();
        let grand: f64 = totals.iter().sum();
        let mut order: Vec<usize> = (0..slices.len()).collect();
        if largest_first {
            order.sort_by(|a, b| totals[*b].total_cmp(&totals[*a]));
        }
        let n = slices.len().max(1);
        let tones = series_tones(&t, parts.len());
        let muted = t.readable_muted(0.85);

        // Scale: pink's three gridlines at 0, ½ and the top.
        let peak = totals.iter().copied().fold(0.0_f64, f64::max);
        let scale = chart_scale(peak);
        let axis_labels = [scale, scale / 2.0, 0.0].map(|v| {
            if peak <= 0.0 && v > 0.0 {
                String::new()
            } else if v == 0.0 {
                "0".into()
            } else {
                measure.fmt(v)
            }
        });

        // Per-column geometry: slot centers glide on a re-sort, part heights
        // (as fractions of the scale) glide between measures and grow in when
        // the dimensions change.
        let dims_key = format!("{key}-{slice_dim}-{seg_dim}");
        let slot_of = |sl: usize| order.iter().position(|o| *o == sl).unwrap_or(sl);
        let columns: Vec<Column> = (0..slices.len())
            .map(|sl| Column {
                x: motion::follow(
                    &format!("{dims_key}-x-{sl}"),
                    (slot_of(sl) as f32 + 0.5) / n as f32,
                    motion::RESORT,
                ),
                fractions: (0..parts.len())
                    .map(|g| {
                        motion::follow_from(
                            &format!("{dims_key}-h-{sl}-{g}"),
                            0.0,
                            (matrix[sl][g] / scale) as f32,
                            motion::EMPHASIZED,
                        )
                    })
                    .collect(),
                hot: motion::follow(
                    &format!("{key}-hot-{sl}"),
                    if hot == Some(sl) { 1.0 } else { 0.0 },
                    motion::HOVER_FADE,
                ),
            })
            .collect();

        // --- View card: pink's "Show" / "Period" rows --------------------
        let measure_control = {
            let state = state.clone();
            pink::segmented(
                format!("{key}-measure"),
                cube.measures
                    .iter()
                    .enumerate()
                    .map(|(ix, m)| Segment::new(ix.to_string(), m.name.clone()))
                    .collect(),
                measure_ix,
                segmented_width(cube.measures.len()),
                ControlSize::Md,
                &t,
                move |ix, _, cx| {
                    state.update(cx, |s, cx| {
                        s.measure = *ix;
                        cx.notify();
                    })
                },
            )
        };
        let dimension_segments: Vec<Segment> = cube
            .dimensions
            .iter()
            .enumerate()
            .map(|(ix, d)| Segment::new(ix.to_string(), d.name.clone()))
            .collect();
        let slice_control = {
            let state = state.clone();
            pink::segmented(
                format!("{key}-slice"),
                dimension_segments.clone(),
                slice_dim,
                segmented_width(dims),
                ControlSize::Md,
                &t,
                move |ix, _, cx| {
                    state.update(cx, |s, cx| {
                        s.choose_slice(*ix);
                        cx.notify();
                    })
                },
            )
        };
        let segment_control = {
            let state = state.clone();
            pink::segmented(
                format!("{key}-segment"),
                dimension_segments,
                seg_dim,
                segmented_width(dims),
                ControlSize::Md,
                &t,
                move |ix, _, cx| {
                    state.update(cx, |s, cx| {
                        s.choose_segment(*ix);
                        cx.notify();
                    })
                },
            )
        };
        let view_card = pink::section_card(&t)
            .mt_0()
            .child(
                pink::card_row(true, &t)
                    .child(pink::row_text(
                        "Show",
                        Some("The figure the chart and the table add up.".into()),
                        &t,
                    ))
                    .child(pink::row_controls().child(measure_control)),
            )
            .child(
                pink::card_row(false, &t)
                    .child(pink::row_text(
                        "Group by",
                        Some("One column, and one table row, per value.".into()),
                        &t,
                    ))
                    .child(pink::row_controls().child(slice_control)),
            )
            .child(
                pink::card_row(false, &t)
                    .child(pink::row_text(
                        "Split by",
                        Some("The parts stacked in each column.".into()),
                        &t,
                    ))
                    .child(pink::row_controls().child(segment_control)),
            );

        // --- Summary card: the total and its parts, then the chart row ----
        // The parts' key: pink's provider-row title line (an 8 px dot in the
        // series tone, gap 6, the name), here as one wrapping line beside
        // the total instead of a row per part, since the Breakdown table
        // already carries every part's figures.
        let parts_key = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .flex_wrap()
            .justify_end()
            .items_center()
            .gap_x(px(12.0))
            .gap_y(px(4.0))
            .children(parts.iter().enumerate().map(|(g, name)| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().flex_none().size(px(8.0)).rounded_full().bg(tones[g]))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(16.0))
                            .text_color(muted)
                            .child(name.clone()),
                    )
            }));
        // pink: the total in Display, forced semibold.
        let total_row = pink::card_row(false, &t)
            .child(
                div()
                    .flex_none()
                    .type_style(TextStyle::Display)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(measure.fmt_total(grand)),
            )
            .child(parts_key);

        // pink: the hovered period's detail and total.
        let hover_detail = hot.filter(|sl| *sl < slices.len()).map(|sl| {
            let values: Vec<String> = (0..parts.len())
                .filter(|g| matrix[sl][*g] > 0.0)
                .map(|g| format!("{} {}", parts[g], measure.fmt(matrix[sl][g])))
                .collect();
            if values.is_empty() {
                format!("{} · No activity", slices[sl])
            } else {
                format!(
                    "{} · {} · Total {}",
                    slices[sl],
                    values.join(" · "),
                    measure.fmt(totals[sl])
                )
            }
        });
        let hover_readout = hot
            .filter(|sl| *sl < slices.len())
            .map(|sl| format!("{} · Total {}", slices[sl], measure.fmt(totals[sl])));
        let chart = self.chart(
            &t,
            &key,
            &state,
            &focus,
            &columns,
            &order,
            &slices,
            &parts,
            &matrix,
            &totals,
            &tones,
            &measure,
            axis_labels,
        );
        // The description becomes the hovered column's detail (pink); it
        // keeps to one line so the chart under it never shifts.
        let chart_description = hover_detail.unwrap_or_else(|| {
            format!(
                "One column per {}, stacked by {}. Hover for values.",
                slice_name.to_lowercase(),
                seg_name.to_lowercase()
            )
        });
        let chart_row = pink::card_row(false, &t)
            .child(
                pink::row_text(
                    format!("{} by {}", measure.name, slice_name.to_lowercase()),
                    None,
                    &t,
                )
                .child(pink::row_description(chart_description, &t).truncate()),
            )
            .when_some(hover_readout, |row, readout| {
                row.child(
                    pink::row_controls().child(
                        div()
                            .type_style(TextStyle::Caption)
                            .text_color(muted)
                            .child(readout),
                    ),
                )
            })
            .child(pink::row_block().child(chart));
        let summary_card = pink::section_card(&t)
            .child(pink::section_header(
                self.title.clone(),
                self.subtitle.clone(),
                &t,
            ))
            .child(total_row)
            .child(chart_row);

        // --- Breakdown card: pink's table under a section header ---------
        let order_control = {
            let state = state.clone();
            pink::segmented(
                format!("{key}-order"),
                vec![
                    Segment::new("listed", "Listed"),
                    Segment::new("largest", "Largest"),
                ],
                usize::from(largest_first),
                segmented_width(2),
                ControlSize::Md,
                &t,
                move |ix, _, cx| {
                    state.update(cx, |s, cx| {
                        s.largest_first = *ix == 1;
                        cx.notify();
                    })
                },
            )
        };
        // pink `usage.rs`: the label takes 40%, the figures share the rest.
        let column_width = 0.60 / (parts.len() + 2) as f32;
        let figure = |value: String, color: Hsla| {
            div()
                .w(relative(column_width))
                .min_w_0()
                .truncate()
                .text_align(TextAlign::Right)
                .text_color(color)
                .child(value)
        };
        let header_row = div()
            .w_full()
            .min_h(px(TABLE_HEADER_HEIGHT))
            .flex()
            .items_center()
            .type_style(TextStyle::Caption)
            .text_color(muted)
            .child(
                div()
                    .w(relative(0.40))
                    .min_w_0()
                    .truncate()
                    .child(slice_name.clone()),
            )
            .children(parts.iter().map(|name| figure(name.to_string(), muted)))
            .child(figure("Total".into(), muted))
            .child(figure("Share".into(), muted));
        let body = div()
            .relative()
            .w_full()
            .h(px(TABLE_ROW_HEIGHT * slices.len() as f32))
            .children((0..slices.len()).map(|sl| {
                let top = motion::follow(
                    &format!("{dims_key}-row-{sl}"),
                    slot_of(sl) as f32 * TABLE_ROW_HEIGHT,
                    motion::RESORT,
                );
                // Rows aren't controls (no plate); the hovered or chart-lit
                // row brightens its secondary figures instead.
                let quiet = motion::mix(muted, t.text, columns[sl].hot);
                let share = if grand > 0.0 { totals[sl] / grand } else { 0.0 };
                let row_state = state.clone();
                div()
                    .id(SharedString::from(format!("{key}-row-{sl}")))
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(top))
                    .h(px(TABLE_ROW_HEIGHT))
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(t.border)
                    .text_size(px(13.0))
                    .line_height(px(18.0))
                    .on_hover(move |hovered, _, cx| set_hot(&row_state, sl, *hovered, cx))
                    .child(
                        div()
                            .w(relative(0.40))
                            .min_w_0()
                            .truncate()
                            .text_color(t.text)
                            .child(slices[sl].clone()),
                    )
                    .children((0..parts.len()).map(|g| figure(measure.fmt(matrix[sl][g]), quiet)))
                    .child(figure(measure.fmt(totals[sl]), t.text))
                    .child(figure(percent(share), quiet))
            }));
        let table = div()
            .w_full()
            .border_t_1()
            .border_color(t.border)
            .px(px(layout::ROW_PAD_X))
            .pt(px(4.0))
            .pb(px(8.0))
            .flex()
            .flex_col()
            .child(header_row)
            .child(body)
            .when(slices.is_empty(), |table| {
                table.child(
                    pink::row_description("Nothing to break down yet.", &t)
                        .border_t_1()
                        .border_color(t.border)
                        .py(px(12.0)),
                )
            });
        let breakdown_card = pink::section_card(&t)
            .child(
                pink::section_header(
                    "Breakdown",
                    Some(
                        format!(
                            "{} per {} and {}, {}.",
                            measure.name,
                            slice_name.to_lowercase(),
                            seg_name.to_lowercase(),
                            if largest_first {
                                "largest first"
                            } else {
                                "as listed"
                            }
                        )
                        .into(),
                    ),
                    &t,
                )
                .child(pink::row_controls().child(order_control)),
            )
            .child(table);

        motion::keep_alive(window);
        // pink: the settings page column, at most 720 wide.
        div()
            .id(self.id.clone())
            .w_full()
            .max_w(px(layout::PAGE_MAX_WIDTH))
            .flex()
            .flex_col()
            .child(view_card)
            .child(summary_card)
            .child(breakdown_card)
            .into_any_element()
    }
}

impl BreakdownChart {
    /// The chart block (pink `usage_chart`): the value gutter and the plot
    /// side by side, the slice labels under the plot.
    #[allow(clippy::too_many_arguments)]
    fn chart(
        &self,
        t: &Pink,
        key: &str,
        state: &gpui::Entity<State>,
        focus: &FocusHandle,
        columns: &[Column],
        order: &[usize],
        slices: &[SharedString],
        parts: &[SharedString],
        matrix: &[Vec<f64>],
        totals: &[f64],
        tones: &[Hsla],
        measure: &Measure,
        axis_labels: [String; 3],
    ) -> AnyElement {
        let n = slices.len().max(1);
        let label_tone = t.readable_muted(0.85);
        let grid = t.border;
        let wash = t.fill(FillTier::Tertiary);
        let paint_columns = columns.to_vec();
        let paint_tones = tones.to_vec();
        let plot = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let width = f32::from(bounds.size.width);
                let height = f32::from(bounds.size.height);
                if width < 2.0 || height < 2.0 {
                    return;
                }
                let at = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
                // pink: 1 px gridlines in `border` at 0, ½ and the top.
                for y in [0.0, (height / 2.0).floor(), height - 1.0] {
                    window.paint_quad(fill(
                        Bounds::new(at(0.0, y), size(px(width), px(1.0))),
                        grid,
                    ));
                }
                let slot = width / n as f32;
                let column_w = (slot * 0.4).min(COLUMN_MAX_WIDTH);
                let rise = height - CHART_HEADROOM;
                for col in &paint_columns {
                    let center = col.x * width;
                    if col.hot > 0.0 {
                        let wash_w = column_w + HOVER_WASH_PAD * 2.0;
                        window.paint_quad(quad(
                            Bounds::new(
                                at(center - wash_w / 2.0, 0.0),
                                size(px(wash_w), px(height)),
                            ),
                            Corners::all(px(radius::SM)),
                            wash.opacity(col.hot),
                            px(0.0),
                            gpui::transparent_black(),
                            Default::default(),
                        ));
                    }
                    let left = center - column_w / 2.0;
                    let mut base = 0.0_f32;
                    for (g, fraction) in col.fractions.iter().enumerate() {
                        let part_h = fraction.max(0.0) * rise;
                        if part_h < 0.5 {
                            continue;
                        }
                        let top = height - base - part_h;
                        let tone = paint_tones[g];
                        window.paint_quad(fill(
                            Bounds::new(at(left, top), size(px(column_w), px(part_h))),
                            tone.opacity(SERIES_BODY_ALPHA),
                        ));
                        window.paint_quad(fill(
                            Bounds::new(
                                at(left, top),
                                size(px(column_w), px(SERIES_LINE.min(part_h))),
                            ),
                            tone,
                        ));
                        base += part_h;
                    }
                }
            },
        )
        .size_full();

        // Hover cells, one per slot (pink: one per period), each with the
        // tooltip pink shows for a period.
        let cells = order.iter().map(|&sl| {
            let detail = std::iter::once(slices[sl].to_string())
                .chain(
                    (0..parts.len())
                        .map(|g| format!("{} {}", parts[g], measure.fmt(matrix[sl][g]))),
                )
                .chain(std::iter::once(format!(
                    "Total {}",
                    measure.fmt(totals[sl])
                )))
                .collect::<Vec<_>>()
                .join("\n");
            let cell_state = state.clone();
            div()
                .id(SharedString::from(format!("{key}-cell-{sl}")))
                .flex_1()
                .h_full()
                .on_hover(move |hovered, _, cx| set_hot(&cell_state, sl, *hovered, cx))
                .tooltip(move |_, cx| {
                    let detail = SharedString::from(detail.clone());
                    cx.new(|_| ChartTooltip(detail)).into()
                })
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
        });

        // Keyboard: the arrows walk the columns in their shown order.
        let key_state = state.clone();
        let key_order = order.to_vec();
        let on_key = move |ev: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            if key_order.is_empty() {
                return;
            }
            let k = ev.keystroke.key.as_str();
            let handled = key_state.update(cx, |s, cx| {
                let pos = s.hot.and_then(|h| key_order.iter().position(|o| *o == h));
                let last = key_order.len() - 1;
                match k {
                    "right" => s.hot = Some(key_order[pos.map_or(0, |p| (p + 1).min(last))]),
                    "left" => s.hot = Some(key_order[pos.map_or(last, |p| p.saturating_sub(1))]),
                    "escape" if s.hot.is_some() => s.hot = None,
                    _ => return false,
                }
                cx.notify();
                true
            });
            if handled {
                cx.stop_propagation();
            }
        };
        let ring = t.focus_ring();
        let focus_click = focus.clone();
        let plot_box = div()
            .id(SharedString::from(format!("{key}-plot")))
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(CHART_HEIGHT))
            .rounded(px(radius::SM))
            .track_focus(focus)
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus_click, cx)
            })
            .on_key_down(on_key)
            .child(plot)
            .child(div().absolute().inset_0().flex().children(cells));

        // Slice labels ride with their columns.
        let x_labels = div()
            .w_full()
            .pl(px(CHART_AXIS_WIDTH + CHART_AXIS_GAP))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(14.0))
                    .type_style(TextStyle::Caption)
                    .children(columns.iter().enumerate().map(|(sl, col)| {
                        let w = 1.0 / n as f32;
                        div()
                            .absolute()
                            .top_0()
                            .left(relative(col.x - w / 2.0))
                            .w(relative(w))
                            .flex()
                            .justify_center()
                            .text_color(motion::mix(label_tone, t.text, col.hot))
                            .child(div().min_w_0().truncate().child(slices[sl].clone()))
                    })),
            );

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
                            .text_align(TextAlign::Right)
                            .type_style(TextStyle::Caption)
                            .text_color(label_tone)
                            .children(axis_labels),
                    )
                    .child(plot_box),
            )
            .child(x_labels)
            .into_any_element()
    }
}

#[derive(Clone)]
struct Column {
    /// Slot center as a fraction of the plot width.
    x: f32,
    /// Each part's height as a fraction of the scale, bottom part first.
    fractions: Vec<f32>,
    /// How lit the column is (hovered here or in the table), 0 to 1.
    hot: f32,
}

/// pink's hover cells: entering lights the slice, leaving clears it only if
/// it is still the lit one.
fn set_hot(state: &gpui::Entity<State>, sl: usize, hovered: bool, cx: &mut App) {
    state.update(cx, |s, cx| {
        let next = if hovered {
            Some(sl)
        } else if s.hot == Some(sl) {
            None
        } else {
            return;
        };
        if s.hot != next {
            s.hot = next;
            cx.notify();
        }
    })
}

/// The empty state (pink's "No usage yet" card): a row tile and row text.
fn empty_card(t: &Pink) -> gpui::Div {
    pink::section_card(t)
        .mt_0()
        .w_full()
        .max_w(px(layout::PAGE_MAX_WIDTH))
        .child(
            pink::card_row(true, t)
                .child(pink::row_tile(ICON_CHART, t))
                .child(pink::row_text(
                    "Nothing to break down yet",
                    Some("Add a measure and at least two dimensions.".into()),
                    t,
                )),
        )
}

/// The chart's hover detail (pink `shell.rs` `SurfaceTabTooltip`): the
/// tooltip card up to 380 wide, `px 8 / py 5`, 11/14 `text`, one line per
/// part.
struct ChartTooltip(SharedString);

impl Render for ChartTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        pink::tooltip_card(&t)
            .max_w(px(380.0))
            .px(px(8.0))
            .py(px(5.0))
            .type_style(TextStyle::Caption)
            .text_color(t.text)
            .child(self.0.clone())
    }
}

/// pink `usage.rs` `count`: thousands separators.
fn count(value: u64) -> String {
    let text = value.to_string();
    let mut grouped = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

/// pink `usage.rs` `money`: "$1,234.56".
fn money(value: f64) -> String {
    let cents = (value.max(0.0) * 100.0).round() as u64;
    format!("${}.{:02}", count(cents / 100), cents % 100)
}

/// pink `usage.rs` `compact_tokens`: "52.2K", "1.25M", "940".
fn compact(value: f64) -> String {
    let value = value.max(0.0).round() as u64;
    for (threshold, suffix) in [
        (1_000_000_000_000, "T"),
        (1_000_000_000, "B"),
        (1_000_000, "M"),
        (1_000, "K"),
    ] {
        if value >= threshold {
            let scaled = value as f64 / threshold as f64;
            let digits = if scaled >= 100.0 {
                0
            } else if scaled >= 10.0 {
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
    count(value)
}

/// pink `usage.rs` `usage_percent`: "28.4%", "<0.1%".
fn percent(value: f64) -> String {
    let percent = value * 100.0;
    if percent > 0.0 && percent < 0.1 {
        "<0.1%".into()
    } else {
        format!("{percent:.1}%")
    }
}

// ---------------------------------------------------------------------------
// Demo: a quarter of cloud spend by team, region, service and stage.
// ---------------------------------------------------------------------------

fn jitter(seed: u32) -> f64 {
    let mut x = seed.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x as f64 / u32::MAX as f64
}

fn names(values: &[&str]) -> Vec<SharedString> {
    values
        .iter()
        .map(|v| SharedString::from(v.to_string()))
        .collect()
}

pub fn demo_cube() -> Cube {
    let teams = ["Platform", "Growth", "Data", "Mobile", "Payments"];
    let regions = ["us-east", "eu-west", "ap-south"];
    let services = ["Compute", "Storage", "Database", "Network"];
    let stages = ["Production", "Staging", "Development"];
    // Quarterly cost per team and requests per team, with the share of each
    // region, service and stage.
    let team_cost = [52_000., 31_000., 44_000., 22_000., 35_000.];
    let team_requests = [820e6, 610e6, 240e6, 940e6, 380e6];
    let region_w = [0.52, 0.31, 0.17];
    let service_cost = [
        [0.46, 0.18, 0.24, 0.12],
        [0.38, 0.12, 0.30, 0.20],
        [0.30, 0.34, 0.28, 0.08],
        [0.42, 0.16, 0.18, 0.24],
        [0.36, 0.10, 0.40, 0.14],
    ];
    let service_req = [0.30, 0.22, 0.33, 0.15];
    let stage_w = [0.72, 0.17, 0.11];
    let mut facts = Vec::new();
    let mut seed = 7;
    for (t, _) in teams.iter().enumerate() {
        for (r, rw) in region_w.iter().enumerate() {
            for (s, _) in services.iter().enumerate() {
                for (e, ew) in stage_w.iter().enumerate() {
                    seed += 1;
                    let j = 0.8 + 0.4 * jitter(seed);
                    let k = 0.8 + 0.4 * jitter(seed + 991);
                    facts.push(Fact {
                        coords: vec![t, r, s, e],
                        values: vec![
                            team_cost[t] * rw * service_cost[t][s] * ew * j,
                            team_requests[t] * rw * service_req[s] * ew * k,
                        ],
                    });
                }
            }
        }
    }
    Cube {
        dimensions: vec![
            Dimension::new("Team", names(&teams)),
            Dimension::new("Region", names(&regions)),
            Dimension::new("Service", names(&services)),
            Dimension::new("Stage", names(&stages)),
        ],
        measures: vec![
            Measure::new("Cost", MeasureFormat::Currency),
            Measure::new("Requests", MeasureFormat::Count),
        ],
        facts,
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    // Teams along the x axis, split by their three regions (pink's three
    // series tones), largest team first.
    story::view(cx, |_, _| {
        BreakdownChart::new("breakdown-chart", "Cloud spend", demo_cube())
            .subtitle("Q3 2026 · all accounts · list prices")
            .measure(0)
            .slice(0)
            .segment(1)
            .largest_first(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_sums_every_fact_once() {
        let cube = demo_cube();
        let total: f64 = cube.facts.iter().map(|f| f.values[0]).sum();
        let m = cube.matrix(0, 0, 2);
        let summed: f64 = m.iter().flatten().sum();
        assert!((total - summed).abs() < 1e-6);
        assert_eq!(m.len(), 5);
        assert_eq!(m[0].len(), 4);
    }

    #[test]
    fn scale_lands_gridlines_on_round_figures() {
        assert_eq!(chart_scale(52_000.), 60_000.);
        assert_eq!(chart_scale(31_000.), 40_000.);
        assert_eq!(chart_scale(940e6), 1e9);
        assert_eq!(chart_scale(0.), 1.);
    }

    #[test]
    fn figures_print_like_pink() {
        let cost = Measure::new("Cost", MeasureFormat::Currency);
        assert_eq!(cost.fmt(52_000.), "$52K");
        assert_eq!(cost.fmt(12_430.), "$12.4K");
        assert_eq!(cost.fmt_total(184_213.524), "$184,213.52");
        let count = Measure::new("Requests", MeasureFormat::Count);
        assert_eq!(count.fmt(1_250_000.), "1.25M");
        assert_eq!(count.fmt(940.), "940");
        assert_eq!(percent(0.2843), "28.4%");
    }
}
