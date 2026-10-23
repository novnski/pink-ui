//! Date Range Picker – a pop-up button that drops a card with period presets
//! on a segmented track, two months side by side, an optional comparison
//! period and Cancel / Apply.
//!
//! Pink has no calendar, so the parts it does have are copied and the grid
//! is drawn in the same language. The trigger is the Usage page's pop-up
//! button (pink `button.rs` `select_frame` + `glyph` + `select_chevron`, as
//! `shell/usage.rs` builds its environment select); the presets are the
//! Usage page's Period control (`shell/usage.rs` `PERIODS` on `button.rs`
//! `segmented`); the card hangs and moves like pink's dropdowns
//! (`popover.rs` `popover_card`, `anchored_menu_below`, `menu_motion_from`);
//! Cancel / Apply are pink's dialog buttons (`btn_ghost` / `btn_primary`);
//! the compare switch is `settings/widgets.rs` `toggle_switch`. Days are
//! plates on the control corner with a selection-wash band, quiet hover
//! fills and the inset focus ring for the keyboard cursor.
//!
//! Click a start day, then an end day (the band follows the pointer in
//! between). The keyboard drives the grid once the card has focus: arrows
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
use web_time::Instant;

use gpui::{
    Anchor, AnyElement, AnyView, App, ClickEvent, Context, DispatchPhase, ElementId, Entity,
    FocusHandle, FontWeight, IntoElement, KeyDownEvent, MouseDownEvent, Role, SharedString, Task,
    Window, anchored, canvas, deferred, div, prelude::*, px,
};

use crate::{
    motion::{
        self, AnimationExt, GLYPH_SWAP, MENU_OUT, MENU_RISE_DOWN, TRANSCRIPT_IN, TRANSCRIPT_IN_RISE,
    },
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, Segment, TextStyle, TypeStyleExt, chrome,
        layout, radius, space,
    },
    story,
};

pub const NAME: &str = "Date Range Picker";
pub const DESCRIPTION: &str = "Pick a reporting window from a pop-up button – period presets on a segmented track, two months whose range band follows the pointer, an optional comparison period and Cancel / Apply, by pointer or keyboard";

/// Pink's `calendar`, `alt-arrow-left` and `alt-arrow-right` glyphs.
const ICON_CALENDAR: &str = "icons/date-range-picker/calendar.svg";
const ICON_PREV: &str = "icons/date-range-picker/alt-arrow-left.svg";
const ICON_NEXT: &str = "icons/date-range-picker/alt-arrow-right.svg";

/// A day plate: a `Md` control's height, on its corner.
const PLATE: f32 = 28.;
/// Day columns: the plate with 2 pt either side for the band to run on.
const CELL_W: f32 = PLATE + 2. * space::S2;
/// Between weeks; the comparison bar sits in it.
const WEEK_GAP: f32 = space::S4;
const MONTH_W: f32 = CELL_W * 7.;
/// The card's content width: two months and the gutter between them.
const CONTENT_W: f32 = MONTH_W * 2. + space::S24;
/// The card's side and top padding.
const INSET: f32 = chrome::CONTENT_INSET;
/// The comparison-mode track: pink's two-option `Sm` segmented (the
/// automation editor's "Each run").
const COMPARE_W: f32 = 220.;

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

impl CompareMode {
    const ALL: [CompareMode; 2] = [CompareMode::PreviousPeriod, CompareMode::PreviousYear];

    pub fn label(self) -> &'static str {
        match self {
            CompareMode::PreviousPeriod => "Previous period",
            CompareMode::PreviousYear => "Previous year",
        }
    }
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

/// Pink's Usage periods (`shell/usage.rs` `PERIODS`) at day grain: today and
/// the last 7, 30 and 90 days.
pub fn default_presets() -> Vec<Preset> {
    vec![
        Preset::last_days("Today", 1),
        Preset::last_days("7 days", 7),
        Preset::last_days("30 days", 30),
        Preset::last_days("90 days", 90),
    ]
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

type ApplyHandler = Rc<dyn Fn(DateRange, Option<DateRange>, &mut Window, &mut App)>;

struct PickerState {
    seeded: bool,
    open: bool,
    /// When the card began closing: it fades out over `MENU_OUT`, then
    /// unmounts (pink `Popup` closing phase).
    closing: Option<Instant>,
    /// Unmounts the card once the exit has run (pink `reap_popup`).
    reap: Option<Task<()>>,
    /// Focus the card on the next frame (pink `pending_menu_focus`).
    pending_focus: bool,
    /// Counts openings, so each one plays the entrance afresh.
    opened: usize,
    trigger: FocusHandle,
    card: FocusHandle,
    applied: DateRange,
    applied_compare: Option<CompareMode>,
    applied_preset: Option<usize>,
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
}

impl PickerState {
    fn new(cx: &mut Context<Self>, today: Day) -> Self {
        let range = DateRange::new(today, today);
        Self {
            seeded: false,
            open: false,
            closing: None,
            reap: None,
            pending_focus: false,
            opened: 0,
            trigger: cx.focus_handle().tab_stop(true),
            card: cx.focus_handle(),
            applied: range,
            applied_compare: None,
            applied_preset: None,
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
        self.preset = self.applied_preset;
        self.cursor = self.applied.end;
        self.hover = None;
        self.keyboard = false;
        self.view = self.applied.end.add_months(-1).first_of_month();
    }

    fn open_card(&mut self) {
        if self.open {
            return;
        }
        self.reset();
        self.open = true;
        self.closing = None;
        self.reap = None;
        self.opened += 1;
        self.pending_focus = true;
    }

    /// Starts the exit; `false` when the card wasn't open.
    fn begin_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        self.pending_focus = false;
        let since = Instant::now();
        self.closing = Some(since);
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        self.reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                if s.closing == Some(since) {
                    s.closing = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        true
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

/// Closes the card; `refocus` hands focus back to the trigger (Escape,
/// Cancel, Apply), a press elsewhere leaves it where the press put it.
fn close(state: &Entity<PickerState>, refocus: bool, window: &mut Window, cx: &mut App) {
    let trigger = state.update(cx, |s, cx| {
        let closed = s.begin_close(cx);
        cx.notify();
        closed.then(|| s.trigger.clone())
    });
    if refocus && let Some(handle) = trigger {
        window.focus(&handle, cx);
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

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

    /// The presets on the card's segmented track (two to four read best;
    /// a Custom segment follows them).
    pub fn presets(mut self, presets: Vec<Preset>) -> Self {
        self.presets = presets;
        self
    }

    /// Starts with the card open (and focused).
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
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let today = self.today;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| PickerState::new(cx, today),
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
                s.applied_preset = preset;
                s.last_compare = compare.unwrap_or(CompareMode::PreviousPeriod);
                s.reset();
                if open {
                    s.open_card();
                }
            });
        }

        // pink `render`: focus the card once it has opened.
        let focus_card = state.update(cx, |s, _| {
            let due = s.pending_focus && s.open;
            if due {
                s.pending_focus = false;
            }
            due.then(|| s.card.clone())
        });
        if let Some(handle) = focus_card {
            window.focus(&handle, cx);
        }

        let (applied, applied_compare, open, closing, opened, trigger_focus) = {
            let s = state.read(cx);
            (
                s.applied,
                s.applied_compare,
                s.open,
                s.closing,
                s.opened,
                s.trigger.clone(),
            )
        };

        // pink `shell/usage.rs` environment select: a Secondary pop-up frame,
        // a leading 14 pt glyph, the choice and the one chevron.
        let label = applied.label();
        let aria: SharedString = match applied_compare {
            Some(mode) => format!("Date range: {label}, compared with the {}", lower(mode)),
            None => format!("Date range: {label}"),
        }
        .into();
        let trigger = pink::select_frame(format!("{key}-trigger"), ControlSize::Md, open, true, &t)
            .track_focus(&trigger_focus)
            .aria_label(aria)
            .on_click({
                let state = state.clone();
                move |_: &ClickEvent, window, cx| {
                    if state.read(cx).open {
                        close(&state, false, window, cx);
                    } else {
                        state.update(cx, |s, cx| {
                            s.open_card();
                            cx.notify();
                        });
                    }
                }
            })
            .child(pink::glyph(ICON_CALENDAR, ControlSize::Md))
            .child(div().flex_none().child(label))
            .when_some(applied_compare, |el, mode| {
                el.child(
                    div()
                        .flex_none()
                        .text_color(t.readable_muted(0.85))
                        .child(format!("vs {}", lower(mode))),
                )
            })
            .child(pink::select_chevron(&t));

        let layer = (open || closing.is_some()).then(|| {
            // Reduced motion drops the fade; the card still unmounts on the
            // exit's clock.
            let reduced = motion::reduced_motion(cx);
            let exit = closing.filter(|_| !open).map(|since| {
                if reduced {
                    1.0
                } else {
                    motion::menu_exit_progress(since)
                }
            });
            let card = card(
                &key,
                &state,
                today,
                &self.presets,
                self.on_apply.clone(),
                &t,
                window,
                cx,
            );
            anchored_below(
                SharedString::from(format!("{key}-card-{opened}")),
                exit,
                card,
            )
        });

        let root = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(trigger)
            .children(layer);
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        root
    }
}

fn lower(mode: CompareMode) -> String {
    mode.label().to_lowercase()
}

/// The card hung from the trigger's bottom-left corner, 6 pt below it and
/// kept 8 pt inside the window (pink `popover::anchored_menu_below`). It
/// drops 6 pt into place on [`motion::MENU_IN`]; while `exit` runs it fades
/// on [`MENU_OUT`] under an occluder so the dying card takes no clicks (pink
/// `menu_motion_from`).
fn anchored_below(id: SharedString, exit: Option<f32>, card: impl IntoElement) -> AnyElement {
    let inner = div()
        .occlude()
        .pt(px(layout::POPOVER_GAP))
        .child(div().relative().child(press_guard()).child(card));
    let layer = match exit {
        Some(t) => motion::menu_out(
            SharedString::from(format!("{id}-out")),
            t,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(id, MENU_RISE_DOWN, inner).into_any_element(),
    };
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .size_0()
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                    .child(layer),
            )
            .priority(1),
        )
        .into_any_element()
}

/// Swallows the press that dismisses the card: the card closes on it during
/// capture, and this stops it in the bubble phase so nothing behind the card
/// (the trigger included) acts on it too (pink `frosted_menu_rounded`'s
/// guard).
fn press_guard() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && !bounds.contains(&event.position) {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

// ---------------------------------------------------------------------------
// Card
// ---------------------------------------------------------------------------

/// The card: pink's `popover_card` without its inset (the panes pad
/// themselves, as pink's model picker does), the presets track, the months,
/// the comparison switch, and under a rule the range beside Cancel / Apply.
#[allow(clippy::too_many_arguments)]
fn card(
    key: &str,
    state: &Entity<PickerState>,
    today: Day,
    presets: &[Preset],
    on_apply: Option<ApplyHandler>,
    t: &Pink,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let s = state.read(cx);
    let focus = s.card.clone();
    let focused = focus.is_focused(window) && s.keyboard;
    let draft = s.draft();
    let picking = s.end.is_none();
    let preview = match (picking, s.hover) {
        (true, Some(h)) if h <= today => Some(DateRange::new(s.start, h)),
        _ => None,
    };
    let shown = preview.unwrap_or(draft);
    let compare = s.compare.map(|m| shown.compared(m));

    // pink `shell/usage.rs` Period: the presets on one segmented track, with
    // a closing Custom segment the thumb rests on for a hand-picked range.
    let ranges: Vec<DateRange> = presets.iter().map(|p| p.range(today)).collect();
    let mut segments: Vec<Segment> = presets
        .iter()
        .enumerate()
        .map(|(ix, p)| Segment::new(ix.to_string(), p.label.clone()))
        .collect();
    segments.push(Segment::new("custom", "Custom"));
    let preset_track = pink::segmented(
        format!("{key}-presets"),
        segments,
        s.preset.unwrap_or(presets.len()),
        CONTENT_W,
        ControlSize::Md,
        t,
        {
            let state = state.clone();
            move |ix: &usize, _, cx| {
                let ix = *ix;
                state.update(cx, |s, cx| {
                    match ranges.get(ix) {
                        Some(range) => s.apply_preset(ix, *range),
                        None => s.preset = None,
                    }
                    cx.notify();
                })
            }
        },
    );

    let left = s.view;
    let right = s.view.add_months(1);
    let slide = s.slide;
    let epoch = s.slide_epoch;
    let weeks = week_rows(left).max(week_rows(right));
    let grid = Grid {
        key,
        state,
        weeks,
        today,
        range: shown,
        previewing: preview.is_some(),
        compare,
        focused,
        t,
    };
    let months = div()
        .flex()
        .gap(px(space::S24))
        .child(grid.month(left, (true, false), cx))
        .child(grid.month(right, (false, true), cx));
    // A page swap: pink's in-place content swap (`TRANSCRIPT_IN`, fade and a
    // 4 pt settle), the settle running the way the months moved.
    let months = if epoch == 0 {
        months.into_any_element()
    } else {
        months
            .with_animation(
                SharedString::from(format!("{key}-months-{epoch}")),
                TRANSCRIPT_IN.animation(),
                move |el, p| {
                    el.relative()
                        .left(px(TRANSCRIPT_IN_RISE * slide * (1. - p)))
                        .opacity(p)
                },
            )
            .into_any_element()
    };

    // The range as it stands over its length and what it is compared with:
    // a settings row's text (pink `settings/widgets.rs` `row_text`, the
    // description as a `meta_line`).
    let (range_text, meta) = if picking {
        (
            format!("{} – …", draft.start.long()),
            vec![div().child("Pick the last day").into_any_element()],
        )
    } else {
        let days = shown.days();
        let mut meta = vec![
            div()
                .child(format!("{days} day{}", if days == 1 { "" } else { "s" }))
                .into_any_element(),
        ];
        if let Some(cmp) = compare {
            meta.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .child(compare_swatch(t))
                    .child(format!("vs {}", cmp.label()))
                    .into_any_element(),
            );
        }
        (shown.label(), meta)
    };
    let summary = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(space::S2))
        .child(pink::row_title(range_text, t))
        .child(meta_line(meta, t));

    // pink `settings/widgets.rs` `toggle_switch` with its label, then the
    // mode on pink's two-option `Sm` track while it is on.
    let compare_on = s.compare.is_some();
    let last_compare = s.last_compare;
    let toggle_compare = {
        let state = state.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                s.compare = match s.compare {
                    Some(_) => None,
                    None => Some(s.last_compare),
                };
                cx.notify();
            })
        }
    };
    let compare_row = div()
        .flex()
        .items_center()
        .gap(px(space::S8))
        .h(px(ControlSize::Sm.height()))
        .child(
            pink::switch_control(format!("{key}-compare"), "Compare", compare_on, t)
                .on_click(toggle_compare.clone()),
        )
        .child(
            div()
                .id(SharedString::from(format!("{key}-compare-label")))
                .flex_none()
                .type_style(TextStyle::Footnote)
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .cursor_pointer()
                .on_click(toggle_compare)
                .child("Compare"),
        )
        .when(compare_on, |row| {
            let state = state.clone();
            row.child(
                div().ml(px(space::S4)).child(pink::segmented(
                    format!("{key}-compare-mode"),
                    CompareMode::ALL
                        .iter()
                        .map(|mode| Segment::new(format!("{mode:?}"), mode.label()))
                        .collect(),
                    CompareMode::ALL
                        .iter()
                        .position(|mode| *mode == last_compare)
                        .unwrap_or(0),
                    COMPARE_W,
                    ControlSize::Sm,
                    t,
                    move |ix: &usize, _, cx| {
                        let mode = CompareMode::ALL[(*ix).min(1)];
                        state.update(cx, |s, cx| {
                            s.compare = Some(mode);
                            s.last_compare = mode;
                            cx.notify();
                        })
                    },
                )),
            )
        });

    // pink `popover.rs` `btn_ghost` / `btn_primary`: the dialog-size pair.
    // Apply waits for the range's second day.
    let cancel = {
        let state = state.clone();
        pink::button(
            format!("{key}-cancel"),
            ButtonKind::Ghost,
            ControlSize::Lg,
            t,
        )
        .aria_label("Cancel")
        .child("Cancel")
        .on_click(move |_, window, cx| close(&state, true, window, cx))
    };
    let apply = {
        let state = state.clone();
        pink::button_enabled(
            format!("{key}-apply"),
            ButtonKind::Primary,
            ControlSize::Lg,
            !picking,
            t,
        )
        .aria_label("Apply")
        .child("Apply")
        .when(!picking, |button| {
            button.on_click(move |_, window, cx| {
                let (range, compare) = state.update(cx, |s, _| {
                    let range = s.draft();
                    s.applied = range;
                    s.applied_compare = s.compare;
                    s.applied_preset = s.preset;
                    (range, s.compare.map(|m| range.compared(m)))
                });
                close(&state, true, window, cx);
                if let Some(f) = &on_apply {
                    f(range, compare, window, cx);
                }
            })
        })
    };

    // Under a rule, the result beside the pair that commits it, as a settings
    // row sets its text beside `row_controls`.
    let footer = div()
        .flex()
        .items_center()
        .gap(px(space::S12))
        .px(px(INSET))
        .py(px(space::S10))
        .border_t_1()
        .border_color(t.separator(false))
        .child(summary)
        .child(pink::row_controls().child(cancel).child(apply));

    let on_key = {
        let state = state.clone();
        move |e: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let key = e.keystroke.key.as_str();
            if key == "escape" {
                close(&state, true, window, cx);
                cx.stop_propagation();
                return;
            }
            // The grid keys belong to the card itself; a focused control
            // inside keeps its own.
            if !state.read(cx).card.is_focused(window) {
                return;
            }
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
                if matches!(key, "enter" | "space") {
                    s.keyboard = true;
                    let cursor = s.cursor;
                    s.pick(cursor, today);
                    cx.notify();
                    return true;
                }
                false
            });
            if handled {
                cx.stop_propagation();
            }
        }
    };

    pink::popover_card_flush(t)
        .id(SharedString::from(format!("{key}-card")))
        .role(Role::Dialog)
        .aria_label("Choose a date range")
        .track_focus(&focus)
        .on_key_down(on_key)
        .on_mouse_down_out({
            let state = state.clone();
            move |_, window, cx| close(&state, false, window, cx)
        })
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(INSET))
                .pt(px(INSET))
                .pb(px(space::S8))
                .child(preset_track),
        )
        .child(div().px(px(INSET)).child(months))
        .child(div().p(px(INSET)).child(compare_row))
        .child(footer)
}

/// The quiet line under the range (pink `settings/widgets.rs` `meta_line`):
/// 12 px fragments in `readable_muted(0.8)`, 8 apart, split by faint dots.
fn meta_line(fragments: Vec<AnyElement>, t: &Pink) -> gpui::Div {
    let dot = if t.dark {
        t.text_muted.opacity(0.3)
    } else {
        t.text_muted
    };
    let mut line = div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(space::S8))
        .type_style(TextStyle::Footnote)
        .text_color(t.readable_muted(0.8));
    for (ix, fragment) in fragments.into_iter().enumerate() {
        if ix > 0 {
            line = line.child(div().flex_none().text_color(dot).child("·"));
        }
        line = line.child(div().flex_none().child(fragment));
    }
    line
}

/// The comparison mark: a 2 pt bar in the muted tone, as under the days.
fn compare_swatch(t: &Pink) -> impl IntoElement {
    div()
        .flex_none()
        .w(px(space::S10))
        .h(px(space::S2))
        .rounded(px(1.))
        .bg(t.text_muted)
}

// ---------------------------------------------------------------------------
// Month grid
// ---------------------------------------------------------------------------

/// What every month and day of one card shares.
struct Grid<'a> {
    key: &'a str,
    state: &'a Entity<PickerState>,
    weeks: usize,
    today: Day,
    /// The committed range, or the preview while the second day follows the
    /// pointer.
    range: DateRange,
    previewing: bool,
    compare: Option<DateRange>,
    focused: bool,
    t: &'a Pink,
}

impl Grid<'_> {
    /// One month: a 24 pt header (pink's `Sm` icon buttons around the title),
    /// the weekday initials and the weeks.
    fn month(&self, first: Day, (with_prev, with_next): (bool, bool), cx: &App) -> AnyElement {
        let t = self.t;
        let s = self.state.read(cx);
        let cursor = s.cursor;
        let pick_epoch = s.pick_epoch;
        let anchor = s.end.is_none().then_some(s.start);
        let title = format!("{} {}", MONTHS[first.month as usize - 1], first.year);
        let nav = |id: &str, icon: &'static str, label: &'static str, delta: i32| {
            let state = self.state.clone();
            pink::icon_button(format!("{}-{id}", self.key), icon, ControlSize::Sm, t)
                .aria_label(label)
                .tooltip(pink::text_tooltip(label))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.page(delta);
                        cx.notify();
                    })
                })
                .into_any_element()
        };
        let spacer = || {
            div()
                .flex_none()
                .size(px(ControlSize::Sm.height()))
                .into_any_element()
        };
        let header = div()
            .flex()
            .items_center()
            .h(px(ControlSize::Sm.height()))
            .mb(px(space::S6))
            .child(if with_prev {
                nav("prev", ICON_PREV, "Previous month", -1)
            } else {
                spacer()
            })
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .type_style(TextStyle::Body)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(title),
            )
            .child(if with_next {
                nav("next", ICON_NEXT, "Next month", 1)
            } else {
                spacer()
            });

        let weekdays = div().flex().mb(px(space::S2)).children(WEEKDAYS.map(|w| {
            div()
                .w(px(CELL_W))
                .h(px(chrome::CONTROL_S))
                .flex()
                .items_center()
                .justify_center()
                .type_style(TextStyle::Caption)
                .text_color(t.readable_faint(0.9))
                .child(w)
        }));

        let lead = first.weekday() as usize;
        let len = days_in_month(first.year, first.month) as usize;
        let mut rows = Vec::new();
        for week in 0..self.weeks {
            let mut row = div().flex();
            for col in 0..7 {
                let slot = week * 7 + col;
                if slot < lead || slot >= lead + len {
                    row = row.child(div().w(px(CELL_W)).h(px(PLATE)));
                    continue;
                }
                let day = Day::new(first.year, first.month, (slot - lead + 1) as u32);
                row = row.child(self.day(
                    day,
                    len,
                    anchor,
                    self.focused && day == cursor,
                    pick_epoch,
                ));
            }
            rows.push(row);
        }

        div()
            .flex()
            .flex_col()
            .w(px(MONTH_W))
            .child(header)
            .child(weekdays)
            .child(div().flex().flex_col().gap(px(WEEK_GAP)).children(rows))
            .into_any_element()
    }

    /// One day: the band behind, the comparison bar in the week gap below,
    /// and the 28 pt plate. Chosen ends are solid (pink's Primary plate),
    /// the end still following the pointer is a selected fill, a plain day
    /// takes the hover fill under the pointer, today keeps a hairline edge
    /// and the keyboard cursor wears the inset focus ring.
    fn day(
        &self,
        day: Day,
        month_len: usize,
        anchor: Option<Day>,
        cursor: bool,
        pick_epoch: usize,
    ) -> impl IntoElement {
        let t = self.t;
        let (today, range, previewing) = (self.today, self.range, self.previewing);
        let id = format!("{}-d-{}-{}-{}", self.key, day.year, day.month, day.day);
        let disabled = day > today;
        let is_today = day == today;
        let in_range = range.contains(day);
        let is_start = day == range.start;
        let is_end = day == range.end;
        let endpoint = is_start || is_end;
        let single = range.start == range.end;
        let solid = endpoint && (!previewing || Some(day) == anchor);
        let row_start = day.weekday() == 0 || day.day == 1;
        let row_end = day.weekday() == 6 || day.day as usize == month_len;
        let side = (CELL_W - PLATE) / 2.;

        // The band: the selection wash (a preview runs in the pressed fill),
        // half under each end and rounded where a week breaks it.
        let band = (in_range && !single).then(|| {
            let color = if previewing {
                t.fill(FillTier::Secondary)
            } else {
                t.accent_wash
            };
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(if is_start {
                    CELL_W / 2.
                } else if row_start {
                    side
                } else {
                    0.
                }))
                .right(px(if is_end {
                    CELL_W / 2.
                } else if row_end {
                    side
                } else {
                    0.
                }))
                .when(row_start && !is_start, |d| d.rounded_l(px(radius::MD)))
                .when(row_end && !is_end, |d| d.rounded_r(px(radius::MD)))
                .bg(color)
        });

        // Only the bar's ends are rounded, so the days' pieces meet flush.
        let compare_mark = self.compare.filter(|c| c.contains(day)).map(|c| {
            let first = day == c.start || row_start;
            let last = day == c.end || row_end;
            div()
                .absolute()
                .bottom(px(-(WEEK_GAP + space::S2) / 2.))
                .h(px(space::S2))
                .left(px(if first { side + space::S4 } else { 0. }))
                .right(px(if last { side + space::S4 } else { 0. }))
                .when(first, |d| d.rounded_l(px(1.)))
                .when(last, |d| d.rounded_r(px(1.)))
                .bg(t.text_muted)
        });

        let hover_key = format!("{id}-hover");
        let hover = t.fill(FillTier::Tertiary);
        let plate = if solid {
            Some(t.text)
        } else if endpoint {
            Some(t.fill(FillTier::Primary))
        } else if disabled {
            None
        } else {
            Some(motion::hover_blend(&hover_key, hover.opacity(0.), hover))
        };
        let number = div()
            .size(px(PLATE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::MD))
            .type_style(TextStyle::Body)
            .when(endpoint || is_today, |d| d.font_weight(FontWeight::MEDIUM))
            .text_color(if solid { t.on_solid } else { t.text })
            .when_some(plate, |d, c| d.bg(c))
            .when(is_today && !solid, |d| {
                d.border_1().border_color(t.separator(true))
            })
            .when(cursor, |d| d.shadow(t.focus_ring_shadows()))
            .when(disabled, |d| d.opacity(0.45))
            .child(day.day.to_string());
        // A chosen end settles in (pink `GLYPH_SWAP`).
        let number = if solid {
            number
                .with_animation(
                    SharedString::from(format!("{id}-pick-{pick_epoch}")),
                    GLYPH_SWAP.animation(),
                    |el, p| el.opacity(0.5 + 0.5 * p),
                )
                .into_any_element()
        } else {
            number.into_any_element()
        };

        let click_state = self.state.clone();
        let hover_state = self.state.clone();
        div()
            .id(SharedString::from(id))
            .relative()
            .w(px(CELL_W))
            .h(px(PLATE))
            .flex()
            .items_center()
            .justify_center()
            .aria_label(day.long())
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
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div().w(px(560.)).h(px(520.)).child(
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
