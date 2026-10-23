//! Date Range Picker – a trigger that opens two months side by side, with
//! preset chips, a live preview while choosing the second date and an
//! optional comparison period drawn under the days.
//!
//! Click a start day, then an end day (the band follows the pointer in
//! between). The keyboard drives it too once the popover has focus: arrows
//! move by day and week, Page Up/Down by month, Home/End to the week's edges,
//! Enter picks, Escape closes.
//!
//! ```ignore
//! DateRangePicker::new("report-range", Day::new(2026, 10, 21))
//!     .value(DateRange::new(Day::new(2026, 9, 28), Day::new(2026, 10, 11)))
//!     .on_apply(|range, compare, _, _| println!("{range:?} vs {compare:?}"))
//! ```

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, AnyView, App, ElementId, Entity, FocusHandle, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseButton, Pixels, Point, SharedString, Window, deferred, div, prelude::*, px,
    svg,
};

use crate::{
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, radius, space, text},
    motion::{self, AnimationExt, MotionSpec},
    story,
};

pub const NAME: &str = "Date Range Picker";
pub const DESCRIPTION: &str = "Two months side by side with preset chips, a live preview band while you pick, an optional comparison period and full keyboard control";

const ICON_CALENDAR: &str = "icons/date-range-picker/calendar.svg";
const ICON_PREV: &str = "icons/date-range-picker/chevron-left.svg";
const ICON_NEXT: &str = "icons/date-range-picker/chevron-right.svg";
const ICON_CHEVRON: &str = "icons/date-range-picker/chevron-down.svg";
const ICON_ARROW: &str = "icons/date-range-picker/arrow-right.svg";

const CELL_W: f32 = 36.;
const CELL_H: f32 = 32.;
const MONTH_GAP: f32 = 28.;
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
/// Month swaps: a short slide with a fade.
const SLIDE: MotionSpec = MotionSpec::new(300, motion::EASE_OUT_QUINT);

// ---------------------------------------------------------------------------
// Calendar math
// ---------------------------------------------------------------------------

/// A calendar day (proleptic Gregorian).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Day {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        let month = month.clamp(1, 12);
        Self {
            year,
            month,
            day: day.clamp(1, days_in_month(year, month)),
        }
    }

    /// Days since 1970-01-01.
    pub fn ordinal(self) -> i64 {
        let (y, m, d) = (self.year as i64, self.month as i64, self.day as i64);
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn from_ordinal(n: i64) -> Self {
        let z = n + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        Self {
            year: y as i32,
            month: m as u32,
            day: d as u32,
        }
    }

    pub fn add_days(self, n: i64) -> Self {
        Self::from_ordinal(self.ordinal() + n)
    }

    /// Same day `n` months away, clamped to the month's length.
    pub fn add_months(self, n: i32) -> Self {
        let index = self.year * 12 + self.month as i32 - 1 + n;
        Self::new(
            index.div_euclid(12),
            index.rem_euclid(12) as u32 + 1,
            self.day,
        )
    }

    /// 0 = Monday … 6 = Sunday.
    pub fn weekday(self) -> u32 {
        (self.ordinal() + 3).rem_euclid(7) as u32
    }

    pub fn first_of_month(self) -> Self {
        Self::new(self.year, self.month, 1)
    }

    fn month_index(self) -> i32 {
        self.year * 12 + self.month as i32 - 1
    }

    fn short(self) -> String {
        format!("{} {}", &MONTHS[self.month as usize - 1][..3], self.day)
    }

    fn long(self) -> String {
        format!("{}, {}", self.short(), self.year)
    }
}

/// Rows a month needs in a Monday-first grid.
fn week_rows(first: Day) -> usize {
    (first.weekday() as usize + days_in_month(first.year, first.month) as usize).div_ceil(7)
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// An inclusive range of days.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateRange {
    pub start: Day,
    pub end: Day,
}

impl DateRange {
    /// Orders the two days.
    pub fn new(a: Day, b: Day) -> Self {
        Self {
            start: a.min(b),
            end: a.max(b),
        }
    }

    pub fn days(self) -> i64 {
        self.end.ordinal() - self.start.ordinal() + 1
    }

    pub fn contains(self, day: Day) -> bool {
        self.start <= day && day <= self.end
    }

    pub fn label(self) -> String {
        if self.start == self.end {
            self.start.long()
        } else if self.start.year == self.end.year {
            format!("{} – {}", self.start.short(), self.end.long())
        } else {
            format!("{} – {}", self.start.long(), self.end.long())
        }
    }

    /// The comparison period for `mode`.
    pub fn compared(self, mode: CompareMode) -> DateRange {
        match mode {
            CompareMode::PreviousPeriod => {
                let end = self.start.add_days(-1);
                DateRange::new(end.add_days(1 - self.days()), end)
            }
            CompareMode::PreviousYear => {
                DateRange::new(self.start.add_months(-12), self.end.add_months(-12))
            }
        }
    }
}

/// What the selection is compared against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareMode {
    PreviousPeriod,
    PreviousYear,
}

/// A named shortcut, computed from "today".
#[derive(Clone)]
pub struct Preset {
    pub label: SharedString,
    range: Rc<dyn Fn(Day) -> DateRange>,
}

impl Preset {
    pub fn new(label: impl Into<SharedString>, range: impl Fn(Day) -> DateRange + 'static) -> Self {
        Self {
            label: label.into(),
            range: Rc::new(range),
        }
    }

    /// The last `n` days, today included.
    pub fn last_days(label: impl Into<SharedString>, n: i64) -> Self {
        Self::new(label, move |today| {
            DateRange::new(today.add_days(1 - n), today)
        })
    }

    pub fn range(&self, today: Day) -> DateRange {
        (self.range)(today)
    }
}

/// Today, the last 7 / 30 / 90 days, month to date and year to date.
pub fn default_presets() -> Vec<Preset> {
    vec![
        Preset::last_days("Today", 1),
        Preset::last_days("7 days", 7),
        Preset::last_days("30 days", 30),
        Preset::last_days("90 days", 90),
        Preset::new("Month to date", |today| {
            DateRange::new(today.first_of_month(), today)
        }),
        Preset::new("Year to date", |today| {
            DateRange::new(Day::new(today.year, 1, 1), today)
        }),
    ]
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

type ApplyHandler = Rc<dyn Fn(DateRange, Option<DateRange>, &mut Window, &mut App)>;

struct PickerState {
    focus: FocusHandle,
    seeded: bool,
    open: bool,
    applied: DateRange,
    applied_compare: Option<CompareMode>,
    start: Day,
    /// `None` while the second date is being chosen.
    end: Option<Day>,
    hover: Option<Day>,
    cursor: Day,
    /// First of the left month.
    view: Day,
    slide: f32,
    slide_epoch: usize,
    preset: Option<usize>,
    compare: Option<CompareMode>,
    last_compare: CompareMode,
    pick_epoch: usize,
    /// The cursor ring only shows once the keyboard has moved it.
    keyboard: bool,
    /// Position of the press that closed the popover from outside, so the
    /// trigger under it doesn't reopen it straight away.
    closed_at: Option<Point<Pixels>>,
}

impl PickerState {
    fn new(window: &mut Window, cx: &mut gpui::Context<Self>, today: Day) -> Self {
        let _ = window;
        let range = DateRange::new(today, today);
        Self {
            focus: cx.focus_handle(),
            seeded: false,
            open: false,
            applied: range,
            applied_compare: None,
            start: today,
            end: Some(today),
            hover: None,
            cursor: today,
            view: today.add_months(-1).first_of_month(),
            slide: 0.,
            slide_epoch: 0,
            preset: None,
            compare: None,
            last_compare: CompareMode::PreviousPeriod,
            pick_epoch: 0,
            keyboard: false,
            closed_at: None,
        }
    }

    fn draft(&self) -> DateRange {
        DateRange::new(self.start, self.end.unwrap_or(self.start))
    }

    /// Resets the draft to what was applied and shows its last month on the right.
    fn reset(&mut self) {
        self.start = self.applied.start;
        self.end = Some(self.applied.end);
        self.compare = self.applied_compare;
        self.cursor = self.applied.end;
        self.hover = None;
        self.view = self.applied.end.add_months(-1).first_of_month();
    }

    fn show(&mut self, day: Day) {
        let left = self.view.month_index();
        let index = day.month_index();
        let target = if index < left {
            Some(day.first_of_month())
        } else if index > left + 1 {
            Some(day.add_months(-1).first_of_month())
        } else {
            None
        };
        if let Some(view) = target {
            self.slide = if view > self.view { 1. } else { -1. };
            self.slide_epoch += 1;
            self.view = view;
        }
    }

    fn page(&mut self, months: i32) {
        self.view = self.view.add_months(months);
        self.slide = months.signum() as f32;
        self.slide_epoch += 1;
    }

    fn pick(&mut self, day: Day, today: Day) {
        if day > today {
            return;
        }
        self.preset = None;
        self.cursor = day;
        self.pick_epoch += 1;
        match self.end {
            Some(_) => {
                self.start = day;
                self.end = None;
                self.hover = Some(day);
            }
            None => {
                let range = DateRange::new(self.start, day);
                self.start = range.start;
                self.end = Some(range.end);
                self.hover = None;
            }
        }
    }

    fn apply_preset(&mut self, ix: usize, range: DateRange) {
        self.preset = Some(ix);
        self.start = range.start;
        self.end = Some(range.end);
        self.cursor = range.end;
        self.hover = None;
        self.pick_epoch += 1;
        self.show(range.end);
        if range.start.month_index() < self.view.month_index() {
            // Show the start too when the range spans two months.
            let view = range.end.add_months(-1).first_of_month();
            if view != self.view {
                self.slide = if view > self.view { 1. } else { -1. };
                self.slide_epoch += 1;
                self.view = view;
            }
        }
    }
}

#[derive(IntoElement)]
pub struct DateRangePicker {
    id: ElementId,
    today: Day,
    value: Option<DateRange>,
    compare: Option<CompareMode>,
    presets: Vec<Preset>,
    open: bool,
    on_apply: Option<ApplyHandler>,
}

impl DateRangePicker {
    /// `today` anchors the presets, marks the current day and caps picking.
    pub fn new(id: impl Into<ElementId>, today: Day) -> Self {
        Self {
            id: id.into(),
            today,
            value: None,
            compare: None,
            presets: default_presets(),
            open: false,
            on_apply: None,
        }
    }

    /// The initial range (defaults to the last 7 days).
    pub fn value(mut self, range: DateRange) -> Self {
        self.value = Some(range);
        self
    }

    /// Starts with a comparison period on.
    pub fn compare(mut self, mode: CompareMode) -> Self {
        self.compare = Some(mode);
        self
    }

    pub fn presets(mut self, presets: Vec<Preset>) -> Self {
        self.presets = presets;
        self
    }

    /// Starts with the popover open.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn on_apply(
        mut self,
        handler: impl Fn(DateRange, Option<DateRange>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_apply = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DateRangePicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let element = self.build(window, cx);
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl DateRangePicker {
    fn build(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let today = self.today;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| PickerState::new(window, cx, today),
        );
        if !state.read(cx).seeded {
            let value = self
                .value
                .unwrap_or_else(|| DateRange::new(today.add_days(-6), today));
            let (compare, open) = (self.compare, self.open);
            let preset = self.presets.iter().position(|p| p.range(today) == value);
            state.update(cx, |s, _| {
                s.seeded = true;
                s.applied = value;
                s.applied_compare = compare;
                s.last_compare = compare.unwrap_or(CompareMode::PreviousPeriod);
                s.open = open;
                s.preset = preset;
                s.reset();
            });
        }

        let s = state.read(cx);
        let applied = s.applied;
        let applied_compare = s.applied_compare.map(|m| applied.compared(m));
        let open = s.open;

        let trigger = {
            let state = state.clone();
            let hover = format!("{key}-trigger");
            let tone = motion::hover_blend(&hover, t.text_muted, t.text);
            div()
                .id(SharedString::from(hover.clone()))
                .flex()
                .items_center()
                .gap(px(space::S8))
                .h(px(34.))
                .pl(px(space::S10))
                .pr(px(space::S8))
                .rounded(px(radius::MD))
                .border_1()
                .border_color(if open {
                    t.accent.opacity(0.55)
                } else {
                    t.border
                })
                .when(open, |d| {
                    d.shadow(vec![crate::primitives::ring(3., t.accent.opacity(0.14))])
                })
                .bg(t.input_bg)
                .cursor_pointer()
                .on_hover(motion::hover_listener(hover))
                .on_mouse_down(MouseButton::Left, move |e, window, cx| {
                    state.update(cx, |s, cx| {
                        if s.closed_at.take() == Some(e.position) {
                            return;
                        }
                        s.open = !s.open;
                        if s.open {
                            s.reset();
                            window.focus(&s.focus, cx);
                        }
                        cx.notify();
                    })
                })
                .child(svg().path(ICON_CALENDAR).size(px(15.)).text_color(tone))
                .child(
                    div()
                        .text_size(px(text::BODY))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(applied.label()),
                )
                .when_some(applied_compare, |d, cmp| {
                    d.child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(format!("vs {}", cmp.label())),
                    )
                })
                .child(luna::badge(
                    format!("{}d", applied.days()),
                    luna::Tone::Neutral,
                    &t,
                ))
                .child(
                    svg()
                        .path(ICON_CHEVRON)
                        .size(px(14.))
                        .text_color(t.text_faint),
                )
        };

        let popover = open.then(|| {
            popover(
                &key,
                &state,
                today,
                &self.presets,
                self.on_apply.clone(),
                &t,
                window,
                cx,
            )
        });

        div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .font_family(luna::SANS)
            .text_color(t.text)
            .child(trigger)
            .when_some(popover, |d, popover| {
                d.child(
                    deferred(div().absolute().top(px(42.)).left_0().child(motion::pop_in(
                        SharedString::from(format!("{key}-pop")),
                        popover,
                    )))
                    .with_priority(1),
                )
            })
            .into_any_element()
    }
}

#[allow(clippy::too_many_arguments)]
fn popover(
    key: &str,
    state: &Entity<PickerState>,
    today: Day,
    presets: &[Preset],
    on_apply: Option<ApplyHandler>,
    t: &Luna,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let s = state.read(cx);
    let focused = s.focus.is_focused(window) && s.keyboard;
    let draft = s.draft();
    let picking = s.end.is_none();
    let preview = match (picking, s.hover) {
        (true, Some(h)) if h <= today => Some(DateRange::new(s.start, h)),
        _ => None,
    };
    let shown = preview.unwrap_or(draft);
    let compare = s.compare.map(|m| shown.compared(m));

    let chips: Vec<_> = presets
        .iter()
        .enumerate()
        .map(|(ix, preset)| {
            let state = state.clone();
            let range = preset.range(today);
            luna::chip(
                format!("{key}-preset-{ix}"),
                s.preset == Some(ix),
                ControlSize::Sm,
                t,
            )
            .child(preset.label.clone())
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.apply_preset(ix, range);
                    cx.notify();
                })
            })
        })
        .collect();

    let left = s.view;
    let right = s.view.add_months(1);
    let slide = s.slide;
    let epoch = s.slide_epoch;
    let weeks = week_rows(left).max(week_rows(right));
    let months = div()
        .flex()
        .gap(px(MONTH_GAP))
        .child(month_view(
            key,
            state,
            left,
            (true, false),
            weeks,
            today,
            shown,
            preview.is_some(),
            compare,
            focused,
            t,
            cx,
        ))
        .child(month_view(
            key,
            state,
            right,
            (false, true),
            weeks,
            today,
            shown,
            preview.is_some(),
            compare,
            focused,
            t,
            cx,
        ));
    let months = if epoch == 0 {
        months.into_any_element()
    } else {
        months
            .with_animation(
                SharedString::from(format!("{key}-months-{epoch}")),
                SLIDE.animation(),
                move |el, p| {
                    el.relative()
                        .left(px(14. * slide * (1. - p)))
                        .opacity(0.25 + 0.75 * p)
                },
            )
            .into_any_element()
    };

    // Footer: the two ends, the comparison switch and the actions.
    let slot = |label: &'static str, value: Option<Day>, active: bool| {
        div()
            .flex()
            .flex_col()
            .w(px(128.))
            .px(px(space::S10))
            .py(px(space::S4))
            .rounded(px(radius::MD))
            .border_1()
            .border_color(if active {
                t.accent.opacity(0.6)
            } else {
                t.border
            })
            .when(active, |d| {
                d.shadow(vec![crate::primitives::ring(2., t.accent.opacity(0.12))])
            })
            .bg(t.input_bg)
            .child(
                div()
                    .text_size(px(text::CAPTION))
                    .line_height(px(14.))
                    .text_color(t.text_faint)
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(text::BODY))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if value.is_some() {
                        t.text
                    } else {
                        t.text_faint
                    })
                    .child(
                        value
                            .map(|d| d.long())
                            .unwrap_or_else(|| "Pick a day".into()),
                    ),
            )
    };
    let end_value = (!picking).then_some(draft.end);

    let compare_on = s.compare.is_some();
    let last_compare = s.last_compare;
    let compare_toggle = {
        let state = state.clone();
        div()
            .id(SharedString::from(format!("{key}-compare")))
            .flex()
            .items_center()
            .gap(px(space::S8))
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.compare = match s.compare {
                        Some(_) => None,
                        None => Some(s.last_compare),
                    };
                    cx.notify();
                })
            })
            .child(switch(&format!("{key}-compare-switch"), compare_on, t))
            .child(
                div()
                    .text_size(px(text::BODY))
                    .text_color(t.text)
                    .child("Compare"),
            )
    };
    let compare_modes = compare_on.then(|| {
        let modes = [
            (CompareMode::PreviousPeriod, "Previous period"),
            (CompareMode::PreviousYear, "Previous year"),
        ];
        div()
            .flex()
            .gap(px(space::S4))
            .children(modes.map(|(mode, label)| {
                let state = state.clone();
                luna::chip(
                    format!("{key}-mode-{label}"),
                    last_compare == mode,
                    ControlSize::Sm,
                    t,
                )
                .child(label)
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.compare = Some(mode);
                        s.last_compare = mode;
                        cx.notify();
                    })
                })
            }))
    });

    let cancel = {
        let state = state.clone();
        luna::button(
            format!("{key}-cancel"),
            ButtonKind::Ghost,
            ControlSize::Md,
            t,
        )
        .child("Cancel")
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                s.reset();
                s.open = false;
                cx.notify();
            })
        })
    };
    let apply = {
        let state = state.clone();
        luna::button(
            format!("{key}-apply"),
            ButtonKind::Primary,
            ControlSize::Md,
            t,
        )
        .child("Apply")
        .on_click(move |_, window, cx| {
            let (range, compare) = state.update(cx, |s, cx| {
                let range = s.draft();
                s.applied = range;
                s.applied_compare = s.compare;
                s.end = Some(range.end);
                s.open = false;
                cx.notify();
                (range, s.compare.map(|m| range.compared(m)))
            });
            if let Some(f) = &on_apply {
                f(range, compare, window, cx);
            }
        })
    };

    let on_key = {
        let state = state.clone();
        move |e: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let key = e.keystroke.key.as_str();
            let handled = state.update(cx, |s, cx| {
                let moved = match key {
                    "left" => Some(s.cursor.add_days(-1)),
                    "right" => Some(s.cursor.add_days(1)),
                    "up" => Some(s.cursor.add_days(-7)),
                    "down" => Some(s.cursor.add_days(7)),
                    "pageup" => Some(s.cursor.add_months(-1)),
                    "pagedown" => Some(s.cursor.add_months(1)),
                    "home" => Some(s.cursor.add_days(-(s.cursor.weekday() as i64))),
                    "end" => Some(s.cursor.add_days(6 - s.cursor.weekday() as i64)),
                    _ => None,
                };
                if let Some(day) = moved {
                    s.keyboard = true;
                    s.cursor = day.min(today);
                    if s.end.is_none() {
                        s.hover = Some(s.cursor);
                    }
                    let cursor = s.cursor;
                    s.show(cursor);
                    cx.notify();
                    return true;
                }
                match key {
                    "enter" | "space" => {
                        let cursor = s.cursor;
                        s.pick(cursor, today);
                    }
                    "escape" => {
                        s.reset();
                        s.open = false;
                    }
                    _ => return false,
                }
                cx.notify();
                true
            });
            if handled {
                cx.stop_propagation();
            }
        }
    };

    let summary = if picking {
        "Now pick the last day".to_string()
    } else {
        format!(
            "{} day{}",
            shown.days(),
            if shown.days() == 1 { "" } else { "s" }
        )
    };

    let close_state = state.clone();
    luna::overlay(t)
        .id(SharedString::from(format!("{key}-popover")))
        .track_focus(&s.focus)
        .on_key_down(on_key)
        .on_mouse_down_out(move |e, _, cx| {
            close_state.update(cx, |s, cx| {
                s.reset();
                s.open = false;
                s.closed_at = Some(e.position);
                cx.notify();
            })
        })
        .flex()
        .flex_col()
        .rounded(px(radius::XL))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(space::S6))
                .px(px(space::S16))
                .pt(px(space::S16))
                .pb(px(space::S12))
                .children(chips),
        )
        .child(luna::divider(t))
        .child(
            div()
                .px(px(space::S16))
                .pt(px(space::S12))
                .pb(px(space::S12))
                .overflow_hidden()
                .child(months),
        )
        .child(luna::divider(t))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::S12))
                .p(px(space::S16))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S8))
                        .child(slot("Start", Some(draft.start), false))
                        .child(
                            svg()
                                .path(ICON_ARROW)
                                .size(px(14.))
                                .text_color(t.text_faint),
                        )
                        .child(slot("End", end_value, picking))
                        .child(
                            div()
                                .ml(px(space::S4))
                                .text_size(px(text::FOOTNOTE))
                                .text_color(if picking { t.accent } else { t.text_muted })
                                .child(summary),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S12))
                        .h(px(28.))
                        .child(compare_toggle)
                        .children(compare_modes)
                        .when_some(compare, |d, cmp| {
                            d.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::S6))
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_muted)
                                    .child(div().w(px(10.)).h(px(2.)).rounded(px(1.)).bg(t.warning))
                                    .child(cmp.label()),
                            )
                        })
                        .child(div().flex_1())
                        .child(cancel)
                        .child(apply),
                ),
        )
}

#[allow(clippy::too_many_arguments)]
fn month_view(
    key: &str,
    state: &Entity<PickerState>,
    first: Day,
    (with_prev, with_next): (bool, bool),
    weeks: usize,
    today: Day,
    range: DateRange,
    previewing: bool,
    compare: Option<DateRange>,
    focused: bool,
    t: &Luna,
    cx: &App,
) -> AnyElement {
    let s = state.read(cx);
    let cursor = s.cursor;
    let pick_epoch = s.pick_epoch;
    let anchor = s.end.is_none().then_some(s.start);
    let title = format!("{} {}", MONTHS[first.month as usize - 1], first.year);
    let nav = |id: &str, icon: &'static str, delta: i32| {
        let state = state.clone();
        luna::icon_button(format!("{key}-{id}"), icon, ControlSize::Sm, t).on_click(
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.page(delta);
                    cx.notify();
                })
            },
        )
    };
    let header = div()
        .flex()
        .items_center()
        .h(px(28.))
        .mb(px(space::S6))
        .child(if with_prev {
            nav("prev", ICON_PREV, -1).into_any_element()
        } else {
            div().w(px(24.)).into_any_element()
        })
        .child(
            div()
                .flex_1()
                .flex()
                .justify_center()
                .text_size(px(text::BODY))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(title),
        )
        .child(if with_next {
            nav("next", ICON_NEXT, 1).into_any_element()
        } else {
            div().w(px(24.)).into_any_element()
        });

    let weekdays = div().flex().children(WEEKDAYS.map(|w| {
        div()
            .w(px(CELL_W))
            .h(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(text::CAPTION))
            .text_color(t.text_faint)
            .child(w)
    }));

    let lead = first.weekday() as usize;
    let len = days_in_month(first.year, first.month) as usize;
    let mut rows = Vec::new();
    for week in 0..weeks {
        let mut row = div().flex();
        for col in 0..7 {
            let slot = week * 7 + col;
            if slot < lead || slot >= lead + len {
                row = row.child(div().w(px(CELL_W)).h(px(CELL_H)));
                continue;
            }
            let day = Day::new(first.year, first.month, (slot - lead + 1) as u32);
            row = row.child(day_cell(
                key,
                state,
                day,
                len,
                today,
                range,
                (previewing, anchor),
                compare,
                focused && day == cursor,
                pick_epoch,
                t,
            ));
        }
        rows.push(row);
    }

    div()
        .flex()
        .flex_col()
        .w(px(CELL_W * 7.))
        .child(header)
        .child(weekdays)
        .child(div().flex().flex_col().gap(px(2.)).children(rows))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn day_cell(
    key: &str,
    state: &Entity<PickerState>,
    day: Day,
    month_len: usize,
    today: Day,
    range: DateRange,
    (previewing, anchor): (bool, Option<Day>),
    compare: Option<DateRange>,
    cursor: bool,
    pick_epoch: usize,
    t: &Luna,
) -> impl IntoElement {
    let id = format!("{key}-d-{}-{}-{}", day.year, day.month, day.day);
    let disabled = day > today;
    let in_range = range.contains(day);
    let is_start = day == range.start;
    let is_end = day == range.end;
    let endpoint = is_start || is_end;
    let single = range.start == range.end;
    // Chosen ends are solid; the end still following the pointer is a plate.
    let solid = endpoint && (!previewing || Some(day) == anchor);
    let week_start = day.weekday() == 0 || day.day == 1;
    let week_end = day.weekday() == 6 || day.day as usize == month_len;

    // The band: a continuous wash with rounded ends at the range and row edges.
    let band = (in_range && !single).then(|| {
        let round_left = is_start || week_start;
        let round_right = is_end || week_end;
        let color = if previewing {
            t.fill(FillTier::Secondary)
        } else {
            t.accent_wash
        };
        div()
            .absolute()
            .top(px(2.))
            .bottom(px(2.))
            .left(px(if is_start { CELL_W / 2. } else { 0. }))
            .right(px(if is_end { CELL_W / 2. } else { 0. }))
            .when(round_left && !is_start, |d| d.rounded_l(px(14.)))
            .when(round_right && !is_end, |d| d.rounded_r(px(14.)))
            .bg(color)
    });

    let compare_mark = compare.filter(|c| c.contains(day)).map(|c| {
        let first = day == c.start || week_start;
        let last = day == c.end || week_end;
        div()
            .absolute()
            .bottom(px(0.))
            .h(px(2.))
            .left(px(if first { 8. } else { 0. }))
            .right(px(if last { 8. } else { 0. }))
            .rounded(px(1.))
            .bg(t.warning.opacity(0.75))
    });

    let hover_key = format!("{id}-hover");
    let hover_t = motion::hover_t(&hover_key);
    let (fg, plate) = if solid {
        (t.on_accent, Some(t.accent_strong))
    } else if endpoint {
        (t.text, Some(t.fill(FillTier::Primary)))
    } else if disabled {
        (t.text_faint.opacity(0.6), None)
    } else if in_range && !previewing {
        (if t.dark { t.text } else { t.accent }, None)
    } else {
        (t.text, None)
    };
    let hover_plate = (!endpoint && !disabled).then(|| t.fill(FillTier::Tertiary).opacity(hover_t));

    let number = div()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_size(px(text::BODY))
        .when(day == today || endpoint, |d| {
            d.font_weight(FontWeight::SEMIBOLD)
        })
        .text_color(fg)
        .when_some(plate, |d, c| d.bg(c))
        .when_some(hover_plate.filter(|_| plate.is_none()), |d, c| d.bg(c))
        .when(cursor, |d| d.border_1().border_color(t.accent))
        .child(day.day.to_string());
    let number = if solid {
        number
            .with_animation(
                SharedString::from(format!("{id}-pick-{pick_epoch}")),
                MotionSpec::new(220, motion::EASE_OUT_QUINT).animation(),
                |el, p| el.opacity(0.4 + 0.6 * p),
            )
            .into_any_element()
    } else {
        number.into_any_element()
    };

    let today_dot = (day == today).then(|| {
        div()
            .absolute()
            .bottom(px(5.))
            .left(px(CELL_W / 2. - 1.5))
            .size(px(3.))
            .rounded_full()
            .bg(if solid { t.on_accent } else { t.accent })
    });

    let click_state = state.clone();
    let hover_state = state.clone();
    div()
        .id(SharedString::from(id))
        .relative()
        .w(px(CELL_W))
        .h(px(CELL_H))
        .flex()
        .items_center()
        .justify_center()
        .when(!disabled, |d| d.cursor_pointer())
        .on_hover(move |hovered, window, cx| {
            motion::hover_listener(hover_key.clone())(hovered, window, cx);
            hover_state.update(cx, |s, cx| {
                if s.end.is_none() {
                    if *hovered {
                        s.hover = Some(day);
                    }
                    cx.notify();
                }
            });
        })
        .on_click(move |_, _, cx| {
            click_state.update(cx, |s, cx| {
                s.keyboard = false;
                s.pick(day, today);
                cx.notify();
            })
        })
        .children(band)
        .children(compare_mark)
        .child(number)
        .children(today_dot)
}

/// A small on/off switch whose knob glides.
fn switch(key: &str, on: bool, t: &Luna) -> impl IntoElement {
    let k = motion::follow(
        &format!("{key}-knob"),
        if on { 1. } else { 0. },
        MotionSpec::new(240, motion::EASE_OUT_QUINT),
    );
    let off: Hsla = t.surface_raised_hover;
    div()
        .w(px(30.))
        .h(px(18.))
        .flex_none()
        .p(px(2.))
        .rounded_full()
        .bg(motion::mix(off, t.accent_strong, k))
        .child(
            div()
                .size(px(14.))
                .ml(px(12. * k))
                .rounded_full()
                .bg(gpui::white())
                .shadow(vec![crate::primitives::shadow(
                    0.,
                    1.,
                    2.,
                    0.,
                    gpui::black().opacity(0.2),
                )]),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div().h(px(500.)).w(px(600.)).child(
            DateRangePicker::new("date-range", Day::new(2026, 10, 21))
                .value(DateRange::new(
                    Day::new(2026, 9, 28),
                    Day::new(2026, 10, 11),
                ))
                .compare(CompareMode::PreviousPeriod)
                .open(true),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_round_trip() {
        assert_eq!(Day::new(1970, 1, 1).ordinal(), 0);
        for n in [-800_000_i64, -1, 0, 59, 365, 19_000, 20_747, 100_000] {
            assert_eq!(Day::from_ordinal(n).ordinal(), n);
        }
        assert_eq!(Day::new(2026, 10, 21).weekday(), 2); // a Wednesday
        assert_eq!(Day::new(2024, 3, 31).add_months(-1), Day::new(2024, 2, 29));
    }

    #[test]
    fn comparison_periods() {
        let range = DateRange::new(Day::new(2026, 10, 11), Day::new(2026, 9, 28));
        assert_eq!(range.days(), 14);
        let prev = range.compared(CompareMode::PreviousPeriod);
        assert_eq!(
            prev,
            DateRange::new(Day::new(2026, 9, 14), Day::new(2026, 9, 27))
        );
        let year = range.compared(CompareMode::PreviousYear);
        assert_eq!(year.start, Day::new(2025, 9, 28));
    }
}
