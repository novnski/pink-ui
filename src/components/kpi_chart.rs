//! KPI Chart – a metric whose headline follows a multi-series trend chart.
//!
//! The card leads with the headline number and its change over the range.
//! Below it, a legend of toggles (one per series, each reading the value
//! under the crosshair) sits over a smoothed line chart. Moving the pointer,
//! or the arrow keys once the chart has focus, walks a crosshair across the
//! points: the headline rolls to that day's total and a readout lists every
//! series. Switching the range morphs the lines into the new shape, and
//! hiding a series rescales the axis.
//!
//! ```ignore
//! KpiChart::new("dau", "Daily active users")
//!     .range(KpiRange::new("30D", dates).series(KpiSeries::new("Web", web)))
//!     .on_range_change(|ix, _, _| println!("range {ix}"))
//! ```

use std::sync::Arc;
use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyView, App, BorderStyle, Bounds, ContentMask, Corners, Edges, ElementId, FocusHandle,
    FontWeight, Hsla, KeyDownEvent, MouseButton, MouseMoveEvent, PathBuilder, Pixels, SharedString,
    Window, canvas, div, fill, linear_color_stop, linear_gradient, point, prelude::*, px, quad,
    size,
};

use crate::{
    luna::{self, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, CubicBezier, MotionSpec},
    story,
};

pub const NAME: &str = "KPI Chart";
pub const DESCRIPTION: &str = "A headline metric over a multi-series trend: a crosshair walks the days, the number rolls with it and a readout splits it by series";

/// Points sampled along each smoothed line; fixed so ranges of any length morph into each other.
const SAMPLES: usize = 120;
const PLOT_H: f32 = 188.;
const AXIS_W: f32 = 44.;
/// Lines draw in from the left on first render.
const DRAW_IN: MotionSpec = MotionSpec::new(1100, motion::EASE_OUT_QUINT).with_delay(120);
/// Range switches and axis rescales.
const MORPH: MotionSpec = MotionSpec::new(520, motion::EASE_OUT_QUINT);
/// The crosshair sliding between points.
const TRACK: MotionSpec = MotionSpec::new(140, CubicBezier::new(0.2, 0.0, 0.0, 1.0));
/// Series fading in and out of focus.
const DIM: MotionSpec = MotionSpec::new(200, motion::EASE);

type RangeHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type Formatter = Rc<dyn Fn(f32) -> String>;

/// One line of the chart.
#[derive(Clone)]
pub struct KpiSeries {
    pub name: SharedString,
    pub values: Vec<f32>,
    /// Overrides the palette color for this series.
    pub color: Option<Hsla>,
}

impl KpiSeries {
    pub fn new(name: impl Into<SharedString>, values: Vec<f32>) -> Self {
        Self {
            name: name.into(),
            values,
            color: None,
        }
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// A selectable time range: its points' labels and the series over them.
/// Every range should list the same series in the same order.
#[derive(Clone)]
pub struct KpiRange {
    pub label: SharedString,
    /// One label per point, e.g. "Oct 12".
    pub points: Vec<SharedString>,
    pub series: Vec<KpiSeries>,
}

impl KpiRange {
    pub fn new(label: impl Into<SharedString>, points: Vec<SharedString>) -> Self {
        Self {
            label: label.into(),
            points,
            series: Vec::new(),
        }
    }

    pub fn series(mut self, series: KpiSeries) -> Self {
        self.series.push(series);
        self
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

    /// A muted line under the headline, e.g. "all platforms".
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

    /// Formats the headline and readout values (default: grouped integers).
    pub fn format(mut self, format: impl Fn(f32) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    /// Formats the axis labels (default: compact, "24k").
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
    hidden: Vec<bool>,
    /// Point under the crosshair.
    hover: Option<usize>,
    /// Series under the pointer in the legend.
    emphasis: Option<usize>,
    plot: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
}

/// The series palette, derived from luna's accent and status colors.
fn palette(t: &Luna) -> [Hsla; 5] {
    // luna's light-mode warning is a deep orange for text; lines read better a step lighter.
    let amber = if t.dark {
        t.warning
    } else {
        luna::oklch(0.70, 0.165, 62.)
    };
    [t.accent, t.success, amber, luna::neutral(0.62), t.danger]
}

impl RenderOnce for KpiChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let default_range = self.default_range;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| State {
                range: default_range,
                hidden: Vec::new(),
                hover: None,
                emphasis: None,
                plot: Rc::new(Cell::new(Bounds::default())),
                focus: cx.focus_handle(),
            },
        );
        if self.ranges.is_empty() {
            return div().id(self.id.clone());
        }
        let (range_ix, hidden, hover, emphasis, plot_cell, focus) = {
            let s = state.read(cx);
            (
                s.range.min(self.ranges.len() - 1),
                s.hidden.clone(),
                s.hover,
                s.emphasis,
                s.plot.clone(),
                s.focus.clone(),
            )
        };
        let range = &self.ranges[range_ix];
        let n = range.points.len().max(1);
        let hover = hover.map(|h| h.min(n - 1));
        let colors = palette(&t);
        let series_color = |ix: usize, s: &KpiSeries| s.color.unwrap_or(colors[ix % colors.len()]);
        let is_hidden = |ix: usize| hidden.get(ix).copied().unwrap_or(false);
        let value = |s: &KpiSeries, i: usize| s.values.get(i).copied().unwrap_or(0.);

        // Headline: the total of the visible series at the crosshair (or the
        // latest point), rolling as it changes.
        let total_at = |i: usize| -> f32 {
            range
                .series
                .iter()
                .enumerate()
                .filter(|(ix, _)| !is_hidden(*ix))
                .map(|(_, s)| value(s, i))
                .sum()
        };
        let focus_ix = hover.unwrap_or(n - 1);
        let headline = motion::follow(&format!("{key}-headline"), total_at(focus_ix), MORPH);
        let first = total_at(0);
        let change = if first > 0. {
            (total_at(focus_ix) - first) / first
        } else {
            0.
        };

        // Scale: a rounded ceiling over the visible series, gliding when it changes.
        let peak = range
            .series
            .iter()
            .enumerate()
            .filter(|(ix, _)| !is_hidden(*ix))
            .flat_map(|(_, s)| s.values.iter().copied())
            .fold(0f32, f32::max);
        let step = nice_step(peak / 4.);
        let y_max = motion::follow(&format!("{key}-ymax"), step * 4., MORPH);
        let reveal = motion::follow_from(&format!("{key}-reveal"), 0., 1., DRAW_IN);

        // Smoothed, resampled lines; each sample glides on its own so a range
        // switch morphs one shape into the next.
        let lines: Vec<Line> = range
            .series
            .iter()
            .enumerate()
            .map(|(ix, s)| {
                let slopes = monotone_slopes(&s.values);
                let samples = (0..SAMPLES)
                    .map(|j| {
                        let u = j as f32 / (SAMPLES - 1) as f32 * (n - 1) as f32;
                        let target = hermite(&s.values, &slopes, u);
                        motion::follow(&format!("{key}-s{ix}-{j}"), target, MORPH)
                    })
                    .collect();
                let target_opacity = if is_hidden(ix) {
                    0.
                } else if emphasis.is_some_and(|e| e != ix) {
                    0.22
                } else {
                    1.
                };
                Line {
                    color: series_color(ix, s),
                    samples,
                    opacity: motion::follow(&format!("{key}-op{ix}"), target_opacity, DIM),
                    filled: emphasis.map_or(ix == 0 && !is_hidden(0), |e| e == ix),
                }
            })
            .collect();

        // Crosshair x as a fraction of the plot width, sliding between points.
        let cross = hover.map(|h| {
            let target = if n > 1 {
                h as f32 / (n - 1) as f32
            } else {
                0.5
            };
            motion::follow(&format!("{key}-cross"), target, TRACK)
        });

        let plot_bounds = plot_cell.get();
        let plot_w = f32::from(plot_bounds.size.width);

        // --- Header -------------------------------------------------------
        let chips = self.ranges.iter().enumerate().map(|(ix, r)| {
            let state = state.clone();
            let handler = self.on_range_change.clone();
            luna::chip(
                format!("{key}-range-{ix}"),
                ix == range_ix,
                ControlSize::Sm,
                &t,
            )
            .font_family(luna::MONO)
            .child(r.label.clone())
            .on_click(move |_, window, cx| {
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
            })
        });
        let caption: SharedString = match hover {
            Some(h) => format!("on {}", range.points.get(h).cloned().unwrap_or_default()).into(),
            None => format!(
                "since {}",
                range.points.first().cloned().unwrap_or_default()
            )
            .into(),
        };
        let delta_tone = if change >= 0. {
            Tone::Success
        } else {
            Tone::Danger
        };
        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(space::S16))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S4))
                    .child(
                        div()
                            .text_size(px(text::BODY))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text_muted)
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S10))
                            .child(
                                div()
                                    .text_size(px(30.))
                                    .line_height(px(36.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.text)
                                    .child((self.format)(headline)),
                            )
                            .child(
                                luna::badge(
                                    format!(
                                        "{} {:.1}%",
                                        if change >= 0. { "▲" } else { "▼" },
                                        change.abs() * 100.
                                    ),
                                    delta_tone,
                                    &t,
                                )
                                .font_family(luna::MONO),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(space::S4))
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(caption)
                            .when_some(self.subtitle.clone(), |d, sub| d.child("·").child(sub)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(space::S2))
                    .p(px(space::S2))
                    .rounded(px(radius::MD))
                    .bg(t.fill(FillTier::Quaternary))
                    .children(chips),
            );

        // --- Legend: one toggle per series --------------------------------
        let legend = div().flex().flex_wrap().gap(px(space::S4)).children(
            range.series.iter().enumerate().map(|(ix, s)| {
                let color = series_color(ix, s);
                let off = is_hidden(ix);
                let hover_key = format!("{key}-legend-{ix}");
                let listener = motion::hover_listener(hover_key.clone());
                let wash = motion::hover_blend(
                    &hover_key,
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                );
                let state_hover = state.clone();
                let state_click = state.clone();
                let count = range.series.len();
                div()
                    .id(SharedString::from(hover_key.clone()))
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .h(px(26.))
                    .px(px(space::S8))
                    .rounded(px(radius::SM))
                    .border_1()
                    .border_color(t.border)
                    .bg(wash)
                    .cursor_pointer()
                    .on_hover(move |hovered, window, cx| {
                        listener(hovered, window, cx);
                        state_hover.update(cx, |st, cx| {
                            let next = if *hovered && !st.hidden.get(ix).copied().unwrap_or(false) {
                                Some(ix)
                            } else if st.emphasis == Some(ix) {
                                None
                            } else {
                                st.emphasis
                            };
                            if next != st.emphasis {
                                st.emphasis = next;
                                cx.notify();
                            }
                        });
                    })
                    .on_click(move |_, _, cx| {
                        state_click.update(cx, |st, cx| {
                            st.hidden.resize(count, false);
                            let visible = st.hidden.iter().filter(|h| !**h).count();
                            // Keep at least one series on the chart.
                            if st.hidden[ix] || visible > 1 {
                                st.hidden[ix] = !st.hidden[ix];
                                if st.hidden[ix] && st.emphasis == Some(ix) {
                                    st.emphasis = None;
                                }
                                cx.notify();
                            }
                        });
                    })
                    .child(
                        div()
                            .size(px(8.))
                            .rounded(px(2.))
                            .border_1()
                            .border_color(color)
                            .when(!off, |d| d.bg(color)),
                    )
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(if off { t.text_faint } else { t.text })
                            .child(s.name.clone()),
                    )
                    .child(
                        div()
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(if off {
                                "–".to_string()
                            } else {
                                (self.axis_format)(value(s, focus_ix))
                            }),
                    )
            }),
        );

        // --- Plot -------------------------------------------------------
        let grid = t.border;
        let hair = t.text_faint.opacity(0.55);
        let card = t.surface_card;
        let paint_lines = lines.clone();
        let plot_canvas = canvas(
            {
                let plot_cell = plot_cell.clone();
                move |bounds, _, _| plot_cell.set(bounds)
            },
            move |bounds, _, window, _| {
                let w = f32::from(bounds.size.width);
                let h = f32::from(bounds.size.height);
                let at = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
                for k in 0..=4 {
                    let y = (h - 1.) * k as f32 / 4.;
                    window.paint_quad(fill(Bounds::new(at(0., y), size(px(w), px(1.))), grid));
                }
                let y_of = |v: f32| h - 1. - (v / y_max.max(1.)).clamp(0., 1.2) * (h - 8.);
                let x_of = |j: usize| w * j as f32 / (SAMPLES - 1) as f32;

                let mask = ContentMask {
                    bounds: Bounds::new(bounds.origin, size(px(w * reveal), px(h))),
                };
                window.with_content_mask(Some(mask), |window| {
                    // Areas first, under every line.
                    for line in paint_lines.iter().rev().filter(|l| l.filled) {
                        let mut area = PathBuilder::fill();
                        area.move_to(at(0., h - 1.));
                        for (j, v) in line.samples.iter().enumerate() {
                            area.line_to(at(x_of(j), y_of(*v)));
                        }
                        area.line_to(at(w, h - 1.));
                        area.close();
                        if let Ok(path) = area.build() {
                            window.paint_path(
                                path,
                                linear_gradient(
                                    180.,
                                    linear_color_stop(line.color.opacity(0.20 * line.opacity), 0.),
                                    linear_color_stop(line.color.opacity(0.), 1.),
                                ),
                            );
                        }
                    }
                    for line in paint_lines.iter().rev() {
                        if line.opacity <= 0.01 {
                            continue;
                        }
                        let mut stroke =
                            PathBuilder::stroke(px(if line.filled { 2. } else { 1.6 }));
                        for (j, v) in line.samples.iter().enumerate() {
                            let p = at(x_of(j), y_of(*v));
                            if j == 0 {
                                stroke.move_to(p);
                            } else {
                                stroke.line_to(p);
                            }
                        }
                        if let Ok(path) = stroke.build() {
                            window.paint_path(path, line.color.opacity(line.opacity));
                        }
                    }
                });

                if let Some(fx) = cross {
                    let x = (w * fx).round();
                    window.paint_quad(fill(Bounds::new(at(x, 0.), size(px(1.), px(h))), hair));
                    for line in paint_lines.iter().filter(|l| l.opacity > 0.3) {
                        let v = sample_at(&line.samples, fx);
                        let c = at(x, y_of(v));
                        let r = 4.5;
                        window.paint_quad(quad(
                            Bounds::new(c - point(px(r), px(r)), size(px(r * 2.), px(r * 2.))),
                            Corners::all(px(r)),
                            line.color.opacity(line.opacity),
                            Edges::all(px(2.)),
                            card,
                            BorderStyle::default(),
                        ));
                    }
                }
            },
        )
        .size_full();

        // Axis labels on the right, aligned to the gridlines.
        let axis = div()
            .relative()
            .w(px(AXIS_W))
            .h(px(PLOT_H))
            .flex_none()
            .children((0..=4).map(|k| {
                let y = (PLOT_H - 1.) * k as f32 / 4.;
                let v = step * (4 - k) as f32;
                div()
                    .absolute()
                    .right_0()
                    .top(px(y - 8.))
                    .text_size(px(text::CAPTION))
                    .line_height(px(16.))
                    .font_family(luna::MONO)
                    .text_color(t.text_faint)
                    .child((self.axis_format)(v))
            }));

        // Dates under the plot: up to five, evenly spread.
        let ticks = tick_indices(n, if n <= 7 { n } else { 5 });
        let dates = div()
            .relative()
            .h(px(16.))
            .mr(px(AXIS_W))
            .mt(px(space::S8))
            .children(ticks.iter().enumerate().map(|(k, &i)| {
                let fx = if n > 1 { i as f32 / (n - 1) as f32 } else { 0. };
                let label = div()
                    .absolute()
                    .top_0()
                    .w(px(64.))
                    .text_size(px(text::CAPTION))
                    .line_height(px(16.))
                    .text_color(if hover == Some(i) {
                        t.text
                    } else {
                        t.text_faint
                    })
                    .child(range.points.get(i).cloned().unwrap_or_default());
                if k == 0 {
                    label.left_0()
                } else if k == ticks.len() - 1 {
                    label.right_0().flex().justify_end()
                } else {
                    label.left(px(plot_w * fx - 32.)).flex().justify_center()
                }
            }));

        // The readout next to the crosshair.
        let readout = hover.zip(cross).map(|(h, fx)| {
            let mut rows: Vec<(usize, &KpiSeries, f32)> = range
                .series
                .iter()
                .enumerate()
                .filter(|(ix, _)| !is_hidden(*ix))
                .map(|(ix, s)| (ix, s, value(s, h)))
                .collect();
            rows.sort_by(|a, b| b.2.total_cmp(&a.2));
            let x = plot_w * fx;
            let tip = div()
                .absolute()
                .top(px(space::S8))
                .w(px(176.))
                .p(px(space::S8))
                .flex()
                .flex_col()
                .gap(px(space::S4))
                .rounded(px(radius::MD))
                .bg(t.surface_overlay)
                .border_1()
                .border_color(t.border)
                .shadow(t.tooltip_shadow())
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .pb(px(space::S2))
                        .text_size(px(text::CAPTION))
                        .text_color(t.text_faint)
                        .child(range.points.get(h).cloned().unwrap_or_default())
                        .child(
                            div()
                                .font_family(luna::MONO)
                                .text_color(t.text_muted)
                                .child((self.format)(total_at(h))),
                        ),
                )
                .children(rows.into_iter().map(|(ix, s, v)| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(18.))
                        .child(div().size(px(6.)).rounded(px(3.)).bg(series_color(ix, s)))
                        .child(
                            div()
                                .flex_1()
                                .text_color(if emphasis == Some(ix) {
                                    t.text
                                } else {
                                    t.text_muted
                                })
                                .child(s.name.clone()),
                        )
                        .child(
                            div()
                                .font_family(luna::MONO)
                                .text_color(t.text)
                                .child((self.format)(v)),
                        )
                }));
            let tip = if fx < 0.62 {
                tip.left(px(x + 14.))
            } else {
                tip.right(px(plot_w - x + 14.))
            };
            motion::fade_in(SharedString::from(format!("{key}-tip")), tip)
        });

        let plot = {
            let move_state = state.clone();
            let leave_state = state.clone();
            let key_state = state.clone();
            let plot_cell = plot_cell.clone();
            let focus_click = focus.clone();
            div()
                .id(SharedString::from(format!("{key}-plot")))
                .relative()
                .flex_1()
                .h(px(PLOT_H))
                .track_focus(&focus)
                .cursor_crosshair()
                .child(plot_canvas)
                .children(readout)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.focus(&focus_click, cx)
                })
                .on_mouse_move(move |ev: &MouseMoveEvent, _, cx| {
                    let b = plot_cell.get();
                    if b.size.width <= px(0.) {
                        return;
                    }
                    let fx = ((ev.position.x - b.origin.x) / b.size.width).clamp(0., 1.);
                    let ix = (fx * (n - 1) as f32).round() as usize;
                    move_state.update(cx, |s, cx| {
                        if s.hover != Some(ix) {
                            s.hover = Some(ix);
                            cx.notify();
                        }
                    });
                })
                .on_hover(move |hovered, window, cx| {
                    if !hovered && !focus_is(&leave_state, window, cx) {
                        leave_state.update(cx, |s, cx| {
                            s.hover = None;
                            cx.notify();
                        });
                    }
                })
                .on_key_down(move |ev: &KeyDownEvent, _, cx| {
                    let step = |s: &mut State, delta: isize| {
                        let cur = s.hover.unwrap_or(n - 1) as isize;
                        let next = if s.hover.is_none() {
                            cur
                        } else {
                            (cur + delta).clamp(0, n as isize - 1)
                        };
                        s.hover = Some(next as usize);
                    };
                    let handled = key_state.update(cx, |s, cx| {
                        match ev.keystroke.key.as_str() {
                            "left" => step(s, -1),
                            "right" => step(s, 1),
                            "home" => s.hover = Some(0),
                            "end" => s.hover = Some(n - 1),
                            "escape" => s.hover = None,
                            _ => return false,
                        }
                        cx.notify();
                        true
                    });
                    if handled {
                        cx.stop_propagation();
                    }
                })
        };

        // Followers started above need frames even when nothing else animates.
        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .w(px(620.))
            .flex()
            .flex_col()
            .gap(px(space::S16))
            .p(px(space::S20))
            .font_family(luna::SANS)
            .child(header)
            .child(legend)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().flex().gap(px(space::S8)).child(plot).child(axis))
                    .child(dates),
            )
    }
}

fn focus_is(state: &gpui::Entity<State>, window: &Window, cx: &App) -> bool {
    state.read(cx).focus.is_focused(window)
}

#[derive(Clone)]
struct Line {
    color: Hsla,
    samples: Vec<f32>,
    opacity: f32,
    filled: bool,
}

/// Fritsch–Carlson tangents: a smooth curve that never overshoots the data.
fn monotone_slopes(y: &[f32]) -> Vec<f32> {
    let n = y.len();
    if n < 2 {
        return vec![0.; n];
    }
    let d: Vec<f32> = y.windows(2).map(|w| w[1] - w[0]).collect();
    let mut m = vec![0.; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0. {
            0.
        } else {
            (d[i - 1] + d[i]) / 2.
        };
    }
    for i in 0..n - 1 {
        if d[i] == 0. {
            m[i] = 0.;
            m[i + 1] = 0.;
            continue;
        }
        let a = m[i] / d[i];
        let b = m[i + 1] / d[i];
        let s = a * a + b * b;
        if s > 9. {
            let tau = 3. / s.sqrt();
            m[i] = tau * a * d[i];
            m[i + 1] = tau * b * d[i];
        }
    }
    m
}

/// The curve through `y` at fractional index `u`.
fn hermite(y: &[f32], m: &[f32], u: f32) -> f32 {
    let n = y.len();
    match n {
        0 => 0.,
        1 => y[0],
        _ => {
            let i = (u.floor() as usize).min(n - 2);
            let t = (u - i as f32).clamp(0., 1.);
            let (t2, t3) = (t * t, t * t * t);
            (2. * t3 - 3. * t2 + 1.) * y[i]
                + (t3 - 2. * t2 + t) * m[i]
                + (-2. * t3 + 3. * t2) * y[i + 1]
                + (t3 - t2) * m[i + 1]
        }
    }
}

/// Linear read of resampled values at fraction `f` of the width.
fn sample_at(samples: &[f32], f: f32) -> f32 {
    if samples.is_empty() {
        return 0.;
    }
    let u = f.clamp(0., 1.) * (samples.len() - 1) as f32;
    let i = (u.floor() as usize).min(samples.len().saturating_sub(2));
    let t = u - i as f32;
    let next = samples.get(i + 1).copied().unwrap_or(samples[i]);
    samples[i] + (next - samples[i]) * t
}

/// A round axis step at or above `raw`.
fn nice_step(raw: f32) -> f32 {
    if raw <= 0. {
        return 1.;
    }
    let base = 10f32.powf(raw.log10().floor());
    let f = raw / base;
    let nice = [1., 1.5, 2., 2.5, 3., 4., 5., 6., 8., 10.]
        .into_iter()
        .find(|c| f <= *c + 1e-4)
        .unwrap_or(10.);
    nice * base
}

/// `count` point indices spread evenly across `n` points.
fn tick_indices(n: usize, count: usize) -> Vec<usize> {
    if n <= 1 || count <= 1 {
        return vec![0];
    }
    (0..count)
        .map(|k| ((k as f32 / (count - 1) as f32) * (n - 1) as f32).round() as usize)
        .collect()
}

/// 48210 → "48,210".
fn grouped(v: f32) -> String {
    let n = v.round() as i64;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// 48210 → "48.2k", 24000 → "24k".
fn compact(v: f32) -> String {
    let a = v.abs();
    if a >= 1_000_000. {
        trim(v / 1_000_000., "M")
    } else if a >= 1_000. {
        trim(v / 1_000., "k")
    } else {
        format!("{}", v.round())
    }
}

fn trim(v: f32, unit: &str) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}{unit}", v.round())
    } else {
        format!("{v:.1}{unit}")
    }
}

// ---------------------------------------------------------------------------
// Demo data: 90 days of daily active users by platform, ending Tue Oct 20.
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

/// "Mon D" for a day of the year 2026.
fn date_label(day_of_year: usize) -> SharedString {
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
            return format!("{name} {d}").into();
        }
        d -= len;
    }
    "Dec 31".into()
}

fn demo_chart() -> KpiChart {
    const DAYS: usize = 90;
    // Oct 20, 2026 is day 293 and a Tuesday.
    let last_day = 293;
    let dates: Vec<SharedString> = (0..DAYS)
        .map(|d| date_label(last_day - (DAYS - 1 - d)))
        .collect();
    let weekend = |d: usize| {
        let weekday = (1 - (DAYS as isize - 1 - d as isize)).rem_euclid(7);
        weekday >= 5
    };
    // (name, start, end, weekend factor, noise)
    let platforms = [
        ("Web", 20_400., 26_800., 0.88, 0.03),
        ("iOS", 12_100., 16_300., 1.04, 0.03),
        ("Android", 8_300., 11_600., 1.03, 0.03),
        ("Desktop", 5_200., 5_900., 0.8, 0.04),
    ];
    let series: Vec<(SharedString, Vec<f32>)> = platforms
        .iter()
        .enumerate()
        .map(|(p, (name, start, end, weekend_factor, noise))| {
            let values = (0..DAYS)
                .map(|d| {
                    let f = d as f32 / (DAYS - 1) as f32;
                    // Growth with a summer lull and a launch bump in late September.
                    let trend = start + (end - start) * (f * f * 0.4 + f * 0.6);
                    let lull = 1. - 0.05 * (-(f - 0.3).powi(2) / 0.01).exp();
                    let launch = 1. + 0.07 * (-(f - 0.74).powi(2) / 0.002).exp();
                    let week = if weekend(d) { *weekend_factor } else { 1. };
                    let j = 1. + (jitter((p * 1000 + d) as u32) - 0.5) * 2. * noise;
                    (trend * lull * launch * week * j).round()
                })
                .collect();
            (SharedString::from(*name), values)
        })
        .collect();
    let range = |label: &str, days: usize| {
        let from = DAYS - days;
        series.iter().fold(
            KpiRange::new(label.to_string(), dates[from..].to_vec()),
            |r, (name, values)| r.series(KpiSeries::new(name.clone(), values[from..].to_vec())),
        )
    };
    KpiChart::new("kpi-chart", "Daily active users")
        .subtitle("all platforms")
        .range(range("7D", 7))
        .range(range("30D", 30))
        .range(range("90D", 90))
        .default_range(1)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| demo_chart())
}
