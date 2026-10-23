//! Breakdown Chart – stacked columns over a small data cube, with a linked table.
//!
//! A sentence sets up the view: "Show [Cost] by [Team] split by [Service]".
//! Each bracket is a picker (a measure or one of the cube's dimensions), and
//! the swap button trades the two dimensions. The columns stack the split's
//! parts; hovering one dims the rest and floats a breakdown card beside it.
//! The table under the chart holds the same numbers and is linked to it:
//! hovering a row lights its column and the other way round, a split header
//! isolates that part in every column, and the Total header sorts both the
//! columns and the rows, which slide into their new places.
//!
//! ```ignore
//! BreakdownChart::new("spend", "Cloud spend", cube)
//!     .slice(0)   // dimension along the x axis
//!     .segment(2) // dimension stacked inside each column
//! ```

use std::{cell::Cell, rc::Rc, sync::Arc};

use gpui::{
    AnyView, App, Bounds, Corners, ElementId, FocusHandle, FontWeight, Hsla, KeyDownEvent,
    MouseButton, MouseMoveEvent, Pixels, SharedString, Window, canvas, deferred, div, fill, point,
    prelude::*, px, quad, size, svg,
};

use crate::{
    HoverFill,
    motion::{self, MotionSpec},
    pink::{self, ControlSize, FillTier, Pink, radius, space, text},
    story,
};

pub const NAME: &str = "Breakdown Chart";
pub const DESCRIPTION: &str = "Stacked columns you set up in a sentence – measure, slice and split – with a floating breakdown on hover and a linked, sortable table";

const ICON_STACK: &str = "icons/breakdown-chart/stack.svg";
const ICON_CHEVRON: &str = "icons/breakdown-chart/chevron-down.svg";
const ICON_CHECK: &str = "icons/breakdown-chart/check.svg";
const ICON_SWAP: &str = "icons/breakdown-chart/swap.svg";
const ICON_SORT: &str = "icons/breakdown-chart/sort.svg";

const PLOT_H: f32 = 196.;
const AXIS_W: f32 = 48.;
const ROW_H: f32 = 32.;
const GROW: MotionSpec = MotionSpec::new(720, motion::EASE_OUT_QUINT);
const SLIDE: MotionSpec = MotionSpec::new(520, motion::EASE_OUT_QUINT);
const DIM: MotionSpec = MotionSpec::new(180, motion::EASE);

/// How a measure prints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureFormat {
    /// "$12.4k"
    Currency,
    /// "1.2M"
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

    /// Like [`Self::fmt`] but always with one decimal, so table columns line up.
    fn fmt_fixed(&self, v: f64) -> String {
        let prefix = if self.format == MeasureFormat::Currency {
            "$"
        } else {
            ""
        };
        let a = v.abs();
        let body = if a >= 1e9 {
            format!("{:.1}B", v / 1e9)
        } else if a >= 1e6 {
            format!("{:.1}M", v / 1e6)
        } else if a >= 1e3 {
            format!("{:.1}k", v / 1e3)
        } else {
            format!("{v:.0}")
        };
        format!("{prefix}{body}")
    }

    fn fmt(&self, v: f64) -> String {
        let prefix = if self.format == MeasureFormat::Currency {
            "$"
        } else {
            ""
        };
        let a = v.abs();
        let body = if a >= 1e9 {
            format!("{}B", trim(v / 1e9))
        } else if a >= 1e6 {
            format!("{}M", trim(v / 1e6))
        } else if a >= 1e3 {
            format!("{}k", trim(v / 1e3))
        } else {
            format!("{v:.0}")
        };
        format!("{prefix}{body}")
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
            values: values,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picker {
    Measure,
    Slice,
    Segment,
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
        }
    }

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
}

struct State {
    measure: usize,
    slice: usize,
    segment: usize,
    sorted: bool,
    /// Slice value under the pointer, in the chart or the table.
    hot: Option<usize>,
    /// Segment value isolated from the table header.
    isolated: Option<usize>,
    open: Option<Picker>,
    /// Highlighted option in the open menu.
    menu_ix: usize,
    /// Picker button under the pointer, so a click on it toggles rather than reopens.
    trigger_hover: Option<Picker>,
    plot: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
}

impl State {
    fn options(&self, picker: Picker, cube: &Cube) -> Vec<SharedString> {
        match picker {
            Picker::Measure => cube.measures.iter().map(|m| m.name.clone()).collect(),
            Picker::Slice | Picker::Segment => {
                cube.dimensions.iter().map(|d| d.name.clone()).collect()
            }
        }
    }

    fn current(&self, picker: Picker) -> usize {
        match picker {
            Picker::Measure => self.measure,
            Picker::Slice => self.slice,
            Picker::Segment => self.segment,
        }
    }

    /// Picks an option; choosing the other axis's dimension swaps the two.
    fn choose(&mut self, picker: Picker, ix: usize) {
        match picker {
            Picker::Measure => self.measure = ix,
            Picker::Slice => {
                if ix == self.segment {
                    self.segment = self.slice;
                }
                self.slice = ix;
            }
            Picker::Segment => {
                if ix == self.slice {
                    self.slice = self.segment;
                }
                self.segment = ix;
            }
        }
        self.hot = None;
        self.isolated = None;
        self.open = None;
    }
}

fn palette(t: &Pink) -> [Hsla; 5] {
    let amber = if t.dark {
        t.warning
    } else {
        pink::oklch(0.70, 0.165, 62.)
    };
    let rose = if t.dark {
        pink::oklch(0.70, 0.17, 5.)
    } else {
        pink::oklch(0.63, 0.20, 8.)
    };
    [t.accent, t.success, amber, rose, pink::neutral(0.62)]
}

impl RenderOnce for BreakdownChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let cube = self.cube.clone();
        let (m0, s0, g0) = (self.measure, self.slice, self.segment);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| State {
                measure: m0,
                slice: s0,
                segment: g0,
                sorted: false,
                hot: None,
                isolated: None,
                open: None,
                menu_ix: 0,
                trigger_hover: None,
                plot: Rc::new(Cell::new(Bounds::default())),
                focus: cx.focus_handle(),
            },
        );
        if cube.dimensions.len() < 2 || cube.measures.is_empty() {
            return pink::card(&t).id(self.id.clone());
        }
        let s = state.read(cx);
        let measure_ix = s.measure.min(cube.measures.len() - 1);
        let (slice_dim, seg_dim) = (s.slice, s.segment);
        let (sorted, hot, isolated, open, menu_ix) =
            (s.sorted, s.hot, s.isolated, s.open, s.menu_ix);
        let plot_cell = s.plot.clone();
        let focus = s.focus.clone();

        let measure = cube.measures[measure_ix].clone();
        let slices = cube.dimensions[slice_dim].values.clone();
        let parts = cube.dimensions[seg_dim].values.clone();
        let matrix = cube.matrix(measure_ix, slice_dim, seg_dim);
        let totals: Vec<f64> = matrix.iter().map(|r| r.iter().sum()).collect();
        let grand: f64 = totals.iter().sum();
        let mut order: Vec<usize> = (0..slices.len()).collect();
        if sorted {
            order.sort_by(|a, b| totals[*b].total_cmp(&totals[*a]));
        }
        let colors = palette(&t);
        let color_of = |g: usize| colors[g % colors.len()];
        let n = slices.len().max(1);

        // Scale: four round steps over the tallest column.
        let peak = totals.iter().copied().fold(0., f64::max) as f32;
        let step = nice_step(peak / 4.);
        let top = motion::follow(&format!("{key}-top"), step * 4., GROW);

        // Per-column geometry, gliding between layouts.
        let dims_key = format!("{key}-{slice_dim}-{seg_dim}");
        let columns: Vec<Column> = (0..slices.len())
            .map(|sl| {
                let slot = order.iter().position(|o| *o == sl).unwrap_or(sl);
                let x = motion::follow(
                    &format!("{dims_key}-x-{sl}"),
                    (slot as f32 + 0.5) / n as f32,
                    SLIDE,
                );
                let heights = (0..parts.len())
                    .map(|g| {
                        motion::follow_from(
                            &format!("{dims_key}-h-{sl}-{g}"),
                            0.,
                            matrix[sl][g] as f32,
                            GROW.with_delay(60 + slot as u64 * 45),
                        )
                    })
                    .collect();
                let lit = hot.is_none_or(|h| h == sl);
                Column {
                    x,
                    heights,
                    opacity: motion::follow(
                        &format!("{key}-col-op-{sl}"),
                        if lit { 1. } else { 0.35 },
                        DIM,
                    ),
                }
            })
            .collect();
        let part_opacity: Vec<f32> = (0..parts.len())
            .map(|g| {
                motion::follow(
                    &format!("{key}-part-op-{g}"),
                    if isolated.is_none_or(|i| i == g) {
                        1.
                    } else {
                        0.18
                    },
                    DIM,
                )
            })
            .collect();
        let part_colors: Vec<Hsla> = (0..parts.len()).map(color_of).collect();

        // --- Header ---------------------------------------------------
        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S10))
                    .child(
                        div()
                            .flex_none()
                            .size(px(32.))
                            .rounded(px(radius::MD))
                            .bg(t.accent_wash)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path(ICON_STACK).size(px(16.)).text_color(if t.dark {
                                t.text
                            } else {
                                t.accent
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(text::CALLOUT))
                                    .line_height(px(18.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text)
                                    .child(self.title.clone()),
                            )
                            .when_some(self.subtitle.clone(), |d, sub| {
                                d.child(
                                    div()
                                        .text_size(px(text::FOOTNOTE))
                                        .line_height(px(16.))
                                        .text_color(t.text_faint)
                                        .child(sub),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        div()
                            .text_size(px(22.))
                            .line_height(px(26.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text)
                            .child(measure.fmt(motion::follow(
                                &format!("{key}-grand"),
                                grand as f32,
                                GROW,
                            ) as f64)),
                    )
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(format!("total {}", measure.name.to_lowercase())),
                    ),
            );

        // --- The sentence ------------------------------------------------
        let token = |picker: Picker, label: SharedString| {
            let tkey = format!("{key}-pick-{picker:?}");
            let listener = motion::hover_listener(tkey.clone());
            let is_open = open == Some(picker);
            let hover_state = state.clone();
            let click_state = state.clone();
            let cube = cube.clone();
            let focus = focus.clone();
            let menu = is_open.then(|| {
                let options = state.read(cx).options(picker, &cube);
                let current = state.read(cx).current(picker);
                let close_state = state.clone();
                deferred(
                    pink::overlay(&t)
                        .id(SharedString::from(format!("{tkey}-menu")))
                        .absolute()
                        .top(px(32.))
                        .left_0()
                        .min_w(px(168.))
                        .p(px(space::S4))
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .on_mouse_down_out(move |_, _, cx| {
                            close_state.update(cx, |s, cx| {
                                if s.trigger_hover != s.open {
                                    s.open = None;
                                    cx.notify();
                                }
                            })
                        })
                        .children(options.into_iter().enumerate().map(|(ix, name)| {
                            let pick_state = state.clone();
                            let row_key = format!("{tkey}-opt-{ix}");
                            let wash = if ix == menu_ix {
                                t.fill(FillTier::Tertiary)
                            } else {
                                t.fill(FillTier::Tertiary).opacity(0.)
                            };
                            let disabled_hint = match picker {
                                Picker::Slice if ix == seg_dim => Some("swap"),
                                Picker::Segment if ix == slice_dim => Some("swap"),
                                _ => None,
                            };
                            div()
                                .id(SharedString::from(row_key))
                                .h(px(28.))
                                .px(px(space::S8))
                                .flex()
                                .items_center()
                                .gap(px(space::S8))
                                .rounded(px(radius::SM))
                                .bg(wash)
                                .cursor_pointer()
                                .text_size(px(text::BODY))
                                .text_color(t.text)
                                .on_hover({
                                    let state = state.clone();
                                    move |h, _, cx| {
                                        if *h {
                                            state.update(cx, |s, cx| {
                                                s.menu_ix = ix;
                                                cx.notify();
                                            })
                                        }
                                    }
                                })
                                .on_click(move |_, _, cx| {
                                    pick_state.update(cx, |s, cx| {
                                        s.choose(picker, ix);
                                        cx.notify();
                                    })
                                })
                                .child(div().flex_1().child(name))
                                .when_some(disabled_hint, |d, hint| {
                                    d.child(
                                        div()
                                            .text_size(px(text::CAPTION))
                                            .text_color(t.text_faint)
                                            .child(hint),
                                    )
                                })
                                .child(svg().path(ICON_CHECK).size(px(14.)).text_color(
                                    if ix == current {
                                        t.accent
                                    } else {
                                        gpui::transparent_black()
                                    },
                                ))
                        })),
                )
                .with_priority(1)
            });
            let rest = t.fill(FillTier::Quaternary);
            let bg = if is_open {
                t.fill(FillTier::Secondary)
            } else {
                motion::hover_blend(&tkey, rest, t.fill(FillTier::Secondary))
            };
            div()
                .relative()
                .child(
                    div()
                        .id(SharedString::from(tkey.clone()))
                        .h(px(26.))
                        .pl(px(space::S8))
                        .pr(px(space::S6))
                        .flex()
                        .items_center()
                        .gap(px(space::S4))
                        .rounded(px(radius::SM))
                        .border_1()
                        .border_color(if is_open { t.border_strong } else { t.border })
                        .bg(bg)
                        .cursor_pointer()
                        .text_size(px(text::BODY))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .on_hover(move |h, window, cx| {
                            listener(h, window, cx);
                            hover_state.update(cx, |s, _| {
                                if *h {
                                    s.trigger_hover = Some(picker);
                                } else if s.trigger_hover == Some(picker) {
                                    s.trigger_hover = None;
                                }
                            })
                        })
                        .on_click(move |_, window, cx| {
                            window.focus(&focus, cx);
                            click_state.update(cx, |s, cx| {
                                if s.open == Some(picker) {
                                    s.open = None;
                                } else {
                                    s.open = Some(picker);
                                    s.menu_ix = s.current(picker);
                                }
                                cx.notify();
                            })
                        })
                        .child(label)
                        .child(
                            svg()
                                .path(ICON_CHEVRON)
                                .size(px(14.))
                                .text_color(t.text_faint),
                        ),
                )
                .children(menu)
        };
        let word = |w: &'static str| {
            div()
                .text_size(px(text::BODY))
                .text_color(t.text_muted)
                .child(w)
        };
        let swap_state = state.clone();
        let sort_state = state.clone();
        let sentence = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .child(word("Show"))
                    .child(token(Picker::Measure, measure.name.clone()))
                    .child(word("by"))
                    .child(token(
                        Picker::Slice,
                        cube.dimensions[slice_dim].name.clone(),
                    ))
                    .child(
                        pink::icon_button(
                            SharedString::from(format!("{key}-swap")),
                            ICON_SWAP,
                            ControlSize::Sm,
                            &t,
                        )
                        .on_click(move |_, _, cx| {
                            swap_state.update(cx, |s, cx| {
                                std::mem::swap(&mut s.slice, &mut s.segment);
                                s.hot = None;
                                s.isolated = None;
                                cx.notify();
                            })
                        }),
                    )
                    .child(word("split by"))
                    .child(token(
                        Picker::Segment,
                        cube.dimensions[seg_dim].name.clone(),
                    )),
            )
            .child(
                pink::chip(
                    SharedString::from(format!("{key}-sort")),
                    sorted,
                    ControlSize::Sm,
                    &t,
                )
                .child(svg().path(ICON_SORT).size(px(12.)).text_color(if sorted {
                    t.accent
                } else {
                    t.text_faint
                }))
                .child("Largest first")
                .on_click(move |_, _, cx| toggle_sort(&sort_state, cx)),
            );

        // --- Chart ------------------------------------------------------
        let grid = t.border;
        let paint_columns = columns.clone();
        let paint_part_opacity = part_opacity.clone();
        let paint_part_colors = part_colors.clone();
        let chart_canvas = canvas(
            {
                let plot_cell = plot_cell.clone();
                move |bounds, _, _| plot_cell.set(bounds)
            },
            move |bounds, _, window, _| {
                let w = f32::from(bounds.size.width);
                let h = f32::from(bounds.size.height);
                let at = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
                for k in 0..=4 {
                    let y = ((h - 1.) * k as f32 / 4.).round();
                    window.paint_quad(fill(Bounds::new(at(0., y), size(px(w), px(1.))), grid));
                }
                let slot = w / n as f32;
                let cw = (slot * 0.4).min(56.);
                for col in &paint_columns {
                    let cx = col.x * w;
                    let mut y = h - 1.;
                    let last = col.heights.iter().rposition(|v| *v > 0.);
                    for (g, v) in col.heights.iter().enumerate() {
                        let ph = v / top.max(1.) * (h - 4.);
                        if ph <= 0.2 {
                            continue;
                        }
                        let gap = if Some(g) == last { 0. } else { 1.5 };
                        let r = if Some(g) == last { 4. } else { 0. };
                        let seg_h = (ph - gap).max(0.5);
                        window.paint_quad(quad(
                            Bounds::new(at(cx - cw / 2., y - ph), size(px(cw), px(seg_h))),
                            Corners {
                                top_left: px(r),
                                top_right: px(r),
                                bottom_left: px(0.),
                                bottom_right: px(0.),
                            },
                            paint_part_colors[g].opacity(col.opacity * paint_part_opacity[g]),
                            px(0.),
                            gpui::transparent_black(),
                            Default::default(),
                        ));
                        y -= ph;
                    }
                }
            },
        )
        .size_full();

        let plot_w = f32::from(plot_cell.get().size.width);
        let breakdown = hot.map(|sl| {
            let fx = columns[sl].x;
            let x = plot_w * fx;
            let cw = (plot_w / n as f32 * 0.4).min(56.);
            let total = totals[sl];
            let card = div()
                .absolute()
                .top(px(space::S4))
                .w(px(204.))
                .p(px(space::S10))
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
                        .pb(px(space::S4))
                        .text_size(px(text::FOOTNOTE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(slices[sl].clone())
                        .child(div().font_family(pink::MONO).child(measure.fmt(total))),
                )
                .children((0..parts.len()).rev().map(|g| {
                    let v = matrix[sl][g];
                    let share = if total > 0. { v / total } else { 0. };
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(18.))
                        .when(isolated.is_some_and(|i| i != g), |d| d.opacity(0.45))
                        .child(div().size(px(8.)).rounded(px(2.)).bg(part_colors[g]))
                        .child(
                            div()
                                .flex_1()
                                .text_color(t.text_muted)
                                .child(parts[g].clone()),
                        )
                        .child(
                            div()
                                .font_family(pink::MONO)
                                .text_color(t.text)
                                .child(measure.fmt(v)),
                        )
                        .child(
                            div()
                                .w(px(32.))
                                .flex()
                                .justify_end()
                                .font_family(pink::MONO)
                                .text_color(t.text_faint)
                                .child(format!("{:.0}%", share * 100.)),
                        )
                }));
            let card = if fx < 0.6 {
                card.left(px(x + cw / 2. + 10.))
            } else {
                card.right(px(plot_w - x + cw / 2. + 10.))
            };
            motion::fade_in(SharedString::from(format!("{key}-tip-{sl}")), card)
        });

        let axis = div()
            .relative()
            .flex_none()
            .w(px(AXIS_W))
            .h(px(PLOT_H))
            .children((0..=4).map(|k| {
                let y = (PLOT_H - 1.) * k as f32 / 4.;
                div()
                    .absolute()
                    .left_0()
                    .top(px(y - 8.))
                    .text_size(px(text::CAPTION))
                    .line_height(px(16.))
                    .font_family(pink::MONO)
                    .text_color(t.text_faint)
                    .child(measure.fmt((step * (4 - k) as f32) as f64))
            }));

        let plot = {
            let move_state = state.clone();
            let leave_state = state.clone();
            let plot_cell = plot_cell.clone();
            let order = order.clone();
            div()
                .id(SharedString::from(format!("{key}-plot")))
                .relative()
                .flex_1()
                .h(px(PLOT_H))
                .child(chart_canvas)
                .children(breakdown)
                .on_mouse_move(move |ev: &MouseMoveEvent, _, cx| {
                    let b = plot_cell.get();
                    if b.size.width <= px(0.) {
                        return;
                    }
                    let fx = ((ev.position.x - b.origin.x) / b.size.width).clamp(0., 0.999);
                    let slot = (fx * n as f32) as usize;
                    let sl = order.get(slot).copied();
                    move_state.update(cx, |s, cx| {
                        if s.hot != sl {
                            s.hot = sl;
                            cx.notify();
                        }
                    })
                })
                .on_hover(move |h, _, cx| {
                    if !h {
                        set_hot(&leave_state, None, cx);
                    }
                })
        };
        let x_labels = div()
            .relative()
            .h(px(18.))
            .ml(px(AXIS_W))
            .mt(px(space::S6))
            .children((0..slices.len()).map(|sl| {
                let slot_w = plot_w / n as f32;
                div()
                    .absolute()
                    .top_0()
                    .left(px(columns[sl].x * plot_w - slot_w / 2.))
                    .w(px(slot_w))
                    .flex()
                    .justify_center()
                    .text_size(px(text::FOOTNOTE))
                    .line_height(px(18.))
                    .text_color(if hot == Some(sl) {
                        t.text
                    } else {
                        t.text_muted
                    })
                    .child(slices[sl].clone())
            }));

        // --- Linked table ------------------------------------------------
        let cell_w = 84.;
        let total_w = 92.;
        let header_cell = |content: gpui::AnyElement| {
            div()
                .flex_none()
                .w(px(cell_w))
                .flex()
                .justify_end()
                .child(content)
        };
        let table_head = div()
            .h(px(30.))
            .px(px(space::S8))
            .flex()
            .items_center()
            .border_b_1()
            .border_color(t.border)
            .text_size(px(text::CAPTION))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.text_faint)
            .child(
                div()
                    .flex_1()
                    .child(cube.dimensions[slice_dim].name.to_uppercase()),
            )
            .children((0..parts.len()).map(|g| {
                let iso_state = state.clone();
                let hkey = format!("{key}-head-{g}");
                let lit = isolated == Some(g);
                header_cell(
                    div()
                        .id(SharedString::from(hkey.clone()))
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .px(px(space::S6))
                        .h(px(22.))
                        .rounded(px(radius::SM))
                        .cursor_pointer()
                        .when(lit, |d| {
                            d.bg(t.fill(FillTier::Secondary)).text_color(t.text)
                        })
                        .when(!lit, |d| {
                            d.hover_fill_fg(
                                hkey.clone(),
                                t.fill(FillTier::Tertiary).opacity(0.),
                                t.fill(FillTier::Tertiary),
                                t.text_faint,
                                t.text,
                            )
                        })
                        .on_click(move |_, _, cx| {
                            iso_state.update(cx, |s, cx| {
                                s.isolated = if s.isolated == Some(g) { None } else { Some(g) };
                                cx.notify();
                            })
                        })
                        .child(div().size(px(8.)).rounded(px(2.)).bg(part_colors[g]))
                        .child(parts[g].to_uppercase())
                        .into_any_element(),
                )
            }))
            .child({
                let sort_state = state.clone();
                let hkey = format!("{key}-head-total");
                div().flex_none().w(px(total_w)).flex().justify_end().child(
                    div()
                        .id(SharedString::from(hkey.clone()))
                        .flex()
                        .items_center()
                        .gap(px(space::S4))
                        .px(px(space::S6))
                        .h(px(22.))
                        .rounded(px(radius::SM))
                        .cursor_pointer()
                        .hover_fill_fg(
                            hkey,
                            t.fill(FillTier::Tertiary).opacity(0.),
                            t.fill(FillTier::Tertiary),
                            if sorted { t.text } else { t.text_faint },
                            t.text,
                        )
                        .on_click(move |_, _, cx| toggle_sort(&sort_state, cx))
                        .child("TOTAL")
                        .child(svg().path(ICON_SORT).size(px(11.)).text_color(if sorted {
                            t.accent
                        } else {
                            t.text_faint
                        })),
                )
            });
        let table_rows = div()
            .relative()
            .h(px(ROW_H * slices.len() as f32))
            .children((0..slices.len()).map(|sl| {
                let slot = order.iter().position(|o| *o == sl).unwrap_or(sl);
                let top =
                    motion::follow(&format!("{dims_key}-row-{sl}"), slot as f32 * ROW_H, SLIDE);
                let row_state = state.clone();
                let lit = hot == Some(sl);
                div()
                    .id(SharedString::from(format!("{key}-row-{sl}")))
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(top))
                    .h(px(ROW_H))
                    .px(px(space::S8))
                    .flex()
                    .items_center()
                    .rounded(px(radius::SM))
                    .bg(if lit {
                        t.fill(FillTier::Tertiary)
                    } else {
                        t.fill(FillTier::Tertiary).opacity(0.)
                    })
                    .on_hover(move |h, _, cx| {
                        if *h {
                            set_hot(&row_state, Some(sl), cx);
                        }
                    })
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(text::BODY))
                            .text_color(t.text)
                            .child(slices[sl].clone()),
                    )
                    .children((0..parts.len()).map(|g| {
                        let focus_cell = isolated == Some(g);
                        div()
                            .flex_none()
                            .w(px(cell_w))
                            .flex()
                            .justify_end()
                            .pr(px(space::S6))
                            .font_family(pink::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(if focus_cell || isolated.is_none() {
                                t.text_muted
                            } else {
                                t.text_faint.opacity(0.6)
                            })
                            .when(focus_cell, |d| d.text_color(t.text))
                            .child(measure.fmt_fixed(matrix[sl][g]))
                    }))
                    .child(
                        div()
                            .flex_none()
                            .w(px(total_w))
                            .flex()
                            .justify_end()
                            .pr(px(space::S6))
                            .font_family(pink::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(measure.fmt_fixed(totals[sl])),
                    )
            }));
        let table = {
            let leave_state = state.clone();
            div()
                .id(SharedString::from(format!("{key}-table")))
                .flex()
                .flex_col()
                .gap(px(space::S4))
                .child(table_head)
                .child(table_rows)
                .on_hover(move |h, _, cx| {
                    if !h {
                        set_hot(&leave_state, None, cx);
                    }
                })
        };

        // Keyboard: the open menu first, then the columns.
        let key_state = state.clone();
        let key_cube = cube.clone();
        let key_order = order.clone();
        let on_key = move |ev: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let k = ev.keystroke.key.as_str();
            let handled = key_state.update(cx, |s, cx| {
                if let Some(picker) = s.open {
                    let len = s.options(picker, &key_cube).len().max(1);
                    match k {
                        "down" => s.menu_ix = (s.menu_ix + 1) % len,
                        "up" => s.menu_ix = (s.menu_ix + len - 1) % len,
                        "enter" | "space" => {
                            let ix = s.menu_ix;
                            s.choose(picker, ix);
                        }
                        "escape" => s.open = None,
                        _ => return false,
                    }
                } else {
                    let pos = s.hot.and_then(|h| key_order.iter().position(|o| *o == h));
                    let last = key_order.len().saturating_sub(1);
                    match k {
                        "right" => s.hot = Some(key_order[pos.map_or(0, |p| (p + 1).min(last))]),
                        "left" => {
                            s.hot = Some(key_order[pos.map_or(last, |p| p.saturating_sub(1))])
                        }
                        "escape" => {
                            s.hot = None;
                            s.isolated = None;
                        }
                        _ => return false,
                    }
                }
                cx.notify();
                true
            });
            if handled {
                cx.stop_propagation();
            }
        };

        motion::keep_alive(window);
        let focus_click = focus.clone();
        pink::card(&t)
            .id(self.id.clone())
            .w(px(780.))
            .p(px(space::S20))
            .flex()
            .flex_col()
            .gap(px(space::S16))
            .font_family(pink::SANS)
            .track_focus(&focus)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus_click, cx)
            })
            .on_key_down(on_key)
            .child(header)
            .child(sentence)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().flex().child(axis).child(plot))
                    .child(x_labels),
            )
            .child(table)
    }
}

#[derive(Clone)]
struct Column {
    /// Slot center as a fraction of the plot width.
    x: f32,
    heights: Vec<f32>,
    opacity: f32,
}

fn set_hot(state: &gpui::Entity<State>, hot: Option<usize>, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.hot != hot {
            s.hot = hot;
            cx.notify();
        }
    })
}

fn toggle_sort(state: &gpui::Entity<State>, cx: &mut App) {
    state.update(cx, |s, cx| {
        s.sorted = !s.sorted;
        cx.notify();
    })
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

fn trim(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round())
    } else {
        format!("{v:.1}")
    }
}

// ---------------------------------------------------------------------------
// Demo: a quarter of cloud spend by team, region, service and environment.
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
    let envs = ["Production", "Staging", "Development"];
    // Quarterly cost per team and requests per team (millions), with the
    // share of each region, service and environment.
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
    let env_w = [0.72, 0.17, 0.11];
    let mut facts = Vec::new();
    let mut seed = 7;
    for (t, _) in teams.iter().enumerate() {
        for (r, rw) in region_w.iter().enumerate() {
            for (s, _) in services.iter().enumerate() {
                for (e, ew) in env_w.iter().enumerate() {
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
            Dimension::new("Environment", names(&envs)),
        ],
        measures: vec![
            Measure::new("Cost", MeasureFormat::Currency),
            Measure::new("Requests", MeasureFormat::Count),
        ],
        facts,
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        BreakdownChart::new("breakdown-chart", "Cloud spend", demo_cube())
            .subtitle("Q3 2026 · all accounts")
            .slice(0)
            .segment(2)
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
    fn choosing_the_other_axis_swaps() {
        let cx_state = |slice, segment| (slice, segment);
        let (mut slice, mut segment) = cx_state(0, 2);
        // Mirrors State::choose for Picker::Slice.
        let ix = 2;
        if ix == segment {
            segment = slice;
        }
        slice = ix;
        assert_eq!((slice, segment), (2, 0));
    }

    #[test]
    fn currency_format() {
        let m = Measure::new("Cost", MeasureFormat::Currency);
        assert_eq!(m.fmt(52_000.), "$52k");
        assert_eq!(m.fmt(12_430.), "$12.4k");
    }
}
