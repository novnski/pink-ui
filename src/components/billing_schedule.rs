//! Billing Schedule – compose a billing plan and see every invoice it makes,
//! in pink's settings section cards: the plan and its line items in one
//! column, the schedule they produce beside them.
//!
//! - **Plan**: the customer's header over two editor groups, as in pink's
//!   automation editor (pink: automations.rs `render_editor`). "Schedule"
//!   holds the start month, the term chips and a one-line summary beside a
//!   clock. "Adjustments" holds a percentage discount and a monthly minimum
//!   that tops small invoices up, each behind a switch.
//! - **Line items**: one card row per item, laid out like the rows of pink's
//!   model price editor (pink: usage.rs `render_usage_price_editor`). Each
//!   row has its cadence pop-up, quantity and amount. "Add item" opens the
//!   catalog menu.
//! - **Invoice schedule**: pink's Usage "Estimated cost" card (pink: usage.rs
//!   `render_usage_page`). It shows the Display total, the Totals grid of
//!   metric cells and the per-invoice area chart with its hover guide,
//!   readout and tooltip.
//!
//! ```ignore
//! BillingSchedule::new("pilot", "Northwind Ops")
//!     .start(2026, 11)
//!     .term(12)
//!     .line(Product::new("seats", "Platform seats", 4_800).unit("per seat").qty(40))
//!     .on_change(|plan, _, _| println!("{} total", plan.total()))
//! ```

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Anchor, AnyElement, AnyView, App, Context, Div, ElementId, Entity, FontWeight, IntoElement,
    PathBuilder, Render, Role, SharedString, Toggled, Window, anchored, canvas, deferred, div,
    point, prelude::*, px, svg,
};
use web_time::Instant;

use crate::{
    motion,
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, TextStyle, Tone, TypeStyleExt as _, layout,
    },
    story,
};

pub const NAME: &str = "Billing Schedule";
pub const DESCRIPTION: &str = "Build a billing plan from a start month, a term, line items on their own cadence, a discount and a monthly minimum, and read its invoices as a total, a grid of figures and a per-invoice cost chart";

const ICON_PREV: &str = "icons/billing-schedule/alt-arrow-left.svg";
const ICON_NEXT: &str = "icons/billing-schedule/alt-arrow-right.svg";
const ICON_PLUS: &str = "icons/billing-schedule/plus.svg";
const ICON_MINUS: &str = "icons/billing-schedule/minus.svg";
const ICON_REMOVE: &str = "icons/billing-schedule/close.svg";
const ICON_SCHEDULE: &str = "icons/billing-schedule/clock-circle.svg";

const TERMS: [usize; 4] = [3, 6, 12, 24];
const DISCOUNTS: [u32; 4] = [5, 10, 15, 20];
const MINIMUM_STEP: i64 = 50_000;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The whole component, the editor column (plan and line items) and the
/// least the schedule column takes beside it. The editor fits a line row's
/// text column (pink's 220) beside its controls without wrapping them.
const WIDTH: f32 = 900.0;
const EDITOR_W: f32 = 564.0;
const SUMMARY_MIN_W: f32 = 300.0;
/// The chart's plot height, value-axis gutter and gutter gap (pink: usage.rs
/// `CHART_HEIGHT`, `CHART_AXIS_WIDTH`, `CHART_AXIS_GAP`).
const CHART_HEIGHT: f32 = 200.0;
const CHART_AXIS_WIDTH: f32 = 56.0;
const CHART_AXIS_GAP: f32 = 8.0;
/// A setting's label column inside an editor group (pink: automations.rs
/// `render_editor` `setting`).
const SETTING_LABEL_W: f32 = 112.0;
/// A line's cadence pop-up and amount column, fixed so the rows' controls
/// line up.
const CADENCE_W: f32 = 88.0;
const AMOUNT_W: f32 = 76.0;
/// A menu that just closed ignores its trigger's click for this long, so the
/// press that dismissed it doesn't reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(250);

/// How often a line item is charged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    Monthly,
    Quarterly,
    Yearly,
    Once,
}

impl Cadence {
    pub const ALL: [Cadence; 4] = [
        Cadence::Monthly,
        Cadence::Quarterly,
        Cadence::Yearly,
        Cadence::Once,
    ];

    fn label(self) -> &'static str {
        match self {
            Cadence::Monthly => "Monthly",
            Cadence::Quarterly => "Quarterly",
            Cadence::Yearly => "Yearly",
            Cadence::Once => "Once",
        }
    }

    /// The price suffix in the catalog menu.
    fn suffix(self) -> &'static str {
        match self {
            Cadence::Monthly => "/ mo",
            Cadence::Quarterly => "/ qtr",
            Cadence::Yearly => "/ yr",
            Cadence::Once => "once",
        }
    }

    /// The cadence menu's trailing note.
    fn every(self) -> &'static str {
        match self {
            Cadence::Monthly => "Every invoice",
            Cadence::Quarterly => "Every 3 months",
            Cadence::Yearly => "Every 12 months",
            Cadence::Once => "First invoice",
        }
    }

    /// Whether the line is charged on the invoice `month` months into the term.
    pub fn due(self, month: usize) -> bool {
        match self {
            Cadence::Monthly => true,
            Cadence::Quarterly => month % 3 == 0,
            Cadence::Yearly => month % 12 == 0,
            Cadence::Once => month == 0,
        }
    }
}

/// A sellable item: catalog entry or plan line.
#[derive(Clone, Debug, PartialEq)]
pub struct Product {
    pub id: SharedString,
    pub name: SharedString,
    /// Price per unit in cents.
    pub price: i64,
    /// What one unit is, e.g. "per seat".
    pub unit: Option<SharedString>,
    pub cadence: Cadence,
    pub qty: u32,
}

impl Product {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        price_cents: i64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            price: price_cents,
            unit: None,
            cadence: Cadence::Monthly,
            qty: 1,
        }
    }

    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn cadence(mut self, cadence: Cadence) -> Self {
        self.cadence = cadence;
        self
    }

    pub fn qty(mut self, qty: u32) -> Self {
        self.qty = qty.max(1);
        self
    }

    /// One charge: the unit price times the quantity, in cents.
    fn charge(&self) -> i64 {
        self.price * self.qty as i64
    }
}

/// The plan as it stands, handed to `on_change`.
#[derive(Clone, Debug, PartialEq)]
pub struct BillingPlan {
    pub start_year: i32,
    /// 1–12.
    pub start_month: u32,
    pub term: usize,
    pub lines: Vec<Product>,
    pub discount: Option<u32>,
    /// Monthly floor in cents.
    pub minimum: Option<i64>,
}

impl BillingPlan {
    /// One amount (cents) per invoice, before the discount and any minimum.
    fn raw(&self) -> Vec<i64> {
        (0..self.term.max(1))
            .map(|m| {
                self.lines
                    .iter()
                    .filter(|l| l.cadence.due(m))
                    .map(Product::charge)
                    .sum()
            })
            .collect()
    }

    fn discounted(&self, raw: i64) -> i64 {
        raw * (100 - self.discount.unwrap_or(0) as i64) / 100
    }

    /// One amount per invoice after the discount and the minimum.
    pub fn invoices(&self) -> Vec<i64> {
        self.raw()
            .into_iter()
            .map(|raw| self.discounted(raw).max(self.minimum.unwrap_or(0)))
            .collect()
    }

    /// Invoices the minimum lifted.
    fn topped_up(&self) -> Vec<bool> {
        self.raw()
            .into_iter()
            .map(|raw| self.minimum.is_some_and(|min| self.discounted(raw) < min))
            .collect()
    }

    pub fn total(&self) -> i64 {
        self.invoices().iter().sum()
    }

    /// What the discount takes off over the term.
    pub fn savings(&self) -> i64 {
        let raw: i64 = self.raw().iter().sum();
        raw - self.discounted(raw)
    }

    /// What one line bills over the whole term, before adjustments.
    fn line_term(&self, line: &Product) -> i64 {
        let charges = (0..self.term.max(1))
            .filter(|m| line.cadence.due(*m))
            .count();
        line.charge() * charges as i64
    }

    fn month_index(&self, offset: usize) -> i32 {
        self.start_year * 12 + self.start_month as i32 - 1 + offset as i32
    }

    /// "Jan 2027".
    fn month_label(&self, offset: usize) -> String {
        let index = self.month_index(offset);
        format!(
            "{} {}",
            MONTHS[index.rem_euclid(12) as usize],
            index.div_euclid(12)
        )
    }

    /// "Jan 1, 2027": invoices go out on the first of the month.
    fn date_label(&self, offset: usize) -> String {
        let index = self.month_index(offset);
        format!(
            "{} 1, {}",
            MONTHS[index.rem_euclid(12) as usize],
            index.div_euclid(12)
        )
    }
}

/// `$12,345.67` (pink: usage.rs `money`, plus a sign for credits).
pub fn money(cents: i64) -> String {
    let sign = if cents < 0 { "−" } else { "" };
    let cents = cents.abs();
    let dollars = (cents / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in dollars.chars().enumerate() {
        if i > 0 && (dollars.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}${grouped}.{:02}", cents % 100)
}

/// Whole-dollar amounts drop their cents (`$2,500`): the chart's axis
/// labels and the minimum stepper.
fn money_short(cents: i64) -> String {
    let full = money(cents);
    match full.strip_suffix(".00") {
        Some(short) => short.to_string(),
        None => full,
    }
}

/// A share as `12.3%` (pink: usage.rs `usage_percent`).
fn percent(value: f64) -> String {
    let percent = value * 100.0;
    if percent > 0.0 && percent < 0.1 {
        "<0.1%".into()
    } else {
        format!("{percent:.1}%")
    }
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

type ChangeHandler = Rc<dyn Fn(&BillingPlan, &mut Window, &mut App)>;

/// The open pop-up: the catalog, or one line's cadence (by line key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Menu {
    Catalog,
    Cadence(u64),
}

struct PlanState {
    seeded: bool,
    plan: BillingPlan,
    keys: Vec<u64>,
    next_key: u64,
    /// Lines up to this key came with the builder; later ones rise in.
    seeded_through: u64,
    last_discount: u32,
    last_minimum: i64,
    menu: Option<Menu>,
    /// The menu that last closed and when: its exit fade and reopen guard.
    closed: Option<(Menu, Instant)>,
    hovered: Option<usize>,
    on_change: Option<ChangeHandler>,
}

impl PlanState {
    fn new() -> Self {
        Self {
            seeded: false,
            plan: BillingPlan {
                start_year: 2026,
                start_month: 1,
                term: 12,
                lines: Vec::new(),
                discount: None,
                minimum: None,
            },
            keys: Vec::new(),
            next_key: 1,
            seeded_through: 0,
            last_discount: 10,
            last_minimum: 250_000,
            menu: None,
            closed: None,
            hovered: None,
            on_change: None,
        }
    }

    fn push_line(&mut self, product: Product) {
        self.plan.lines.push(product);
        self.keys.push(self.next_key);
        self.next_key += 1;
    }

    fn close_menu(&mut self) {
        if let Some(menu) = self.menu.take() {
            self.closed = Some((menu, Instant::now()));
        }
    }

    fn toggle_menu(&mut self, menu: Menu) {
        if self.menu == Some(menu) {
            self.close_menu();
            return;
        }
        let just_closed = self
            .closed
            .is_some_and(|(m, at)| m == menu && at.elapsed() < REOPEN_GUARD);
        if !just_closed {
            self.close_menu();
            self.menu = Some(menu);
        }
    }

    /// Whether `menu` paints, and its exit progress while it fades out.
    fn menu_phase(&self, menu: Menu) -> Option<Option<f32>> {
        if self.menu == Some(menu) {
            return Some(None);
        }
        self.closed
            .filter(|(m, _)| *m == menu)
            .map(|(_, at)| motion::menu_exit_progress(at))
            .filter(|t| *t < 1.0)
            .map(Some)
    }
}

/// Applies `f` to the plan, then tells the owner.
fn edit(
    state: &Entity<PlanState>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut PlanState),
) {
    let (plan, handler) = state.update(cx, |s, cx| {
        f(s);
        cx.notify();
        (s.plan.clone(), s.on_change.clone())
    });
    if let Some(handler) = handler {
        handler(&plan, window, cx);
    }
}

#[derive(IntoElement)]
pub struct BillingSchedule {
    id: ElementId,
    customer: SharedString,
    caption: SharedString,
    plan: BillingPlan,
    catalog: Vec<Product>,
    on_change: Option<ChangeHandler>,
}

impl BillingSchedule {
    pub fn new(id: impl Into<ElementId>, customer: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            customer: customer.into(),
            caption: SharedString::default(),
            plan: BillingPlan {
                start_year: 2026,
                start_month: 1,
                term: 12,
                lines: Vec::new(),
                discount: None,
                minimum: None,
            },
            catalog: Vec::new(),
            on_change: None,
        }
    }

    /// A line under the customer, e.g. the deal name.
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn start(mut self, year: i32, month: u32) -> Self {
        self.plan.start_year = year;
        self.plan.start_month = month.clamp(1, 12);
        self
    }

    /// Term length in months.
    pub fn term(mut self, months: usize) -> Self {
        self.plan.term = months.max(1);
        self
    }

    pub fn line(mut self, product: Product) -> Self {
        self.plan.lines.push(product);
        self
    }

    /// Products offered by "Add item".
    pub fn catalog(mut self, products: impl IntoIterator<Item = Product>) -> Self {
        self.catalog.extend(products);
        self
    }

    pub fn discount(mut self, percent: u32) -> Self {
        self.plan.discount = Some(percent.min(100));
        self
    }

    /// Monthly floor in cents.
    pub fn minimum(mut self, cents: i64) -> Self {
        self.plan.minimum = Some(cents.max(0));
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&BillingPlan, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for BillingSchedule {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let element = self.build(window, cx);
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl BillingSchedule {
    fn build(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| PlanState::new(),
        );
        let on_change = self.on_change.clone();
        let seed = (!state.read(cx).seeded).then(|| self.plan.clone());
        state.update(cx, |s, _| {
            s.on_change = on_change;
            if let Some(plan) = seed {
                s.seeded = true;
                if let Some(d) = plan.discount {
                    s.last_discount = d;
                }
                if let Some(m) = plan.minimum {
                    s.last_minimum = m;
                }
                s.plan = BillingPlan {
                    lines: Vec::new(),
                    ..plan.clone()
                };
                for line in plan.lines {
                    s.push_line(line);
                }
                s.seeded_through = s.next_key - 1;
            }
        });

        let s = state.read(cx);
        // A closing menu keeps painting until its fade ends; nothing else
        // re-renders this component meanwhile, so ask for the frames.
        let menus = [Menu::Catalog]
            .into_iter()
            .chain(s.keys.iter().map(|k| Menu::Cadence(*k)));
        if menus
            .into_iter()
            .any(|m| matches!(s.menu_phase(m), Some(Some(_))))
        {
            window.request_animation_frame();
        }

        // Each column's first card sits at its top edge; the cards below keep
        // pink's section gap.
        let editor = div()
            .flex_none()
            .w(px(EDITOR_W))
            .flex()
            .flex_col()
            .child(plan_card(&key, &state, s, &self.customer, &self.caption, &t).mt(px(0.0)))
            .child(lines_card(&key, &state, s, &self.catalog, &t));
        let summary = div()
            .flex_1()
            .min_w(px(SUMMARY_MIN_W))
            .flex()
            .flex_col()
            .child(schedule_card(&key, &state, s, &t).mt(px(0.0)));

        div()
            .id(self.id.clone())
            .w_full()
            .max_w(px(WIDTH))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(pink::space::S16))
            .font_family(pink::SANS)
            .text_color(t.text)
            .text_size(px(13.0))
            .child(editor)
            .child(summary)
            .into_any_element()
    }
}

// ---------------------------------------------------------------------------
// Plan card
// ---------------------------------------------------------------------------

/// An editor group: its name over its controls, one hairline above (pink:
/// automations.rs `render_editor` `group`).
fn group(id: String, title: &'static str, t: &Pink) -> gpui::Stateful<Div> {
    pink::card_row(false, t)
        .id(SharedString::from(id))
        .flex_col()
        .items_start()
        .gap(px(10.0))
        .child(
            div()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.readable_muted(0.9))
                .child(title),
        )
}

/// A setting inside a group: its label on a fixed column, the control beside
/// it, both wrapping in a narrow column (pink: automations.rs
/// `render_editor` `setting`).
fn setting(label: &'static str, control: impl IntoElement, t: &Pink) -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(px(12.0))
        .gap_y(px(6.0))
        .child(
            div()
                .flex_none()
                .w(px(SETTING_LABEL_W))
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(t.text)
                .child(label),
        )
        .child(div().min_w_0().child(control))
}

/// A choice chip (pink: automations.rs `render_editor` `choice`, a small
/// `button::chip` announced as a radio button).
fn choice(id: String, label: String, on: bool, t: &Pink) -> gpui::Stateful<Div> {
    pink::chip(id, on, ControlSize::Sm, t)
        .role(Role::RadioButton)
        .aria_label(SharedString::from(label.clone()))
        .child(label)
}

/// A row of choice chips (pink's kind chips: a wrapping row, gap 6).
fn choices() -> Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(6.0))
}

/// A switch and what it does, on one line (pink: automations.rs "Archive run
/// chats": the switch, gap 8, a shrinking row description).
fn switch_line(control: gpui::Stateful<Div>, extra: Option<Div>, note: String, t: &Pink) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .child(control)
        .children(extra)
        .child(pink::row_description(note, t).flex_shrink(1.0))
}

fn plan_card(
    key: &str,
    state: &Entity<PlanState>,
    s: &PlanState,
    customer: &SharedString,
    caption: &SharedString,
    t: &Pink,
) -> Div {
    let plan = &s.plan;
    let term = plan.term.max(1);
    let header = pink::section_header(
        customer.clone(),
        (!caption.is_empty()).then(|| caption.clone()),
        t,
    )
    .child(
        pink::row_controls().child(pink::badge("Draft", Tone::Neutral, t).line_height(px(14.0))),
    );

    // Schedule: the start month, the term, and what they mean in words
    // (pink's schedule summary beside CLOCK_CIRCLE).
    let start = {
        let state = state.clone();
        stepper(
            &format!("{key}-start"),
            plan.month_label(0),
            64.0,
            (ICON_PREV, ICON_NEXT),
            ("Previous month".into(), "Next month".into()),
            (true, true),
            t,
            move |delta, window, cx| {
                edit(&state, window, cx, |s| {
                    let index = s.plan.month_index(0) + delta;
                    s.plan.start_year = index.div_euclid(12);
                    s.plan.start_month = index.rem_euclid(12) as u32 + 1;
                })
            },
        )
    };
    let mut terms = TERMS.to_vec();
    if !terms.contains(&term) {
        terms.push(term);
        terms.sort_unstable();
    }
    let terms = choices().children(terms.into_iter().map(|months| {
        let state = state.clone();
        choice(
            format!("{key}-term-{months}"),
            format!("{months} mo"),
            term == months,
            t,
        )
        .on_click(move |_, window, cx| edit(&state, window, cx, |s| s.plan.term = months))
    }));
    let words = if term == 1 {
        format!("One invoice, on {}", plan.date_label(0))
    } else {
        format!(
            "Monthly on the 1st, {} to {} · {term} invoices",
            plan.month_label(0),
            plan.month_label(term - 1)
        )
    };
    let summary = div()
        .id(SharedString::from(format!("{key}-schedule-summary")))
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(
            svg()
                .path(ICON_SCHEDULE)
                .size(px(14.0))
                .flex_none()
                .text_color(t.accent),
        )
        .child(
            div()
                .min_w_0()
                .text_size(px(13.0))
                .line_height(px(18.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(words),
        );
    let schedule = group(format!("{key}-schedule"), "Schedule", t)
        .child(setting("Starts", start, t))
        .child(setting("Term", terms, t))
        .child(summary);

    // Adjustments: each behind a switch; its choices show while it's on.
    let savings = plan.savings();
    let discount = {
        let toggle = state.clone();
        let on = plan.discount.is_some();
        let chips = on.then(|| {
            choices().children(DISCOUNTS.map(|pct| {
                let state = state.clone();
                choice(
                    format!("{key}-pct-{pct}"),
                    format!("{pct}%"),
                    plan.discount == Some(pct),
                    t,
                )
                .on_click(move |_, window, cx| {
                    edit(&state, window, cx, |s| {
                        s.plan.discount = Some(pct);
                        s.last_discount = pct;
                    })
                })
            }))
        });
        let note = if on {
            format!("Saves {} over the term", money(savings))
        } else {
            "Take a share off every invoice".into()
        };
        switch_line(
            pink::switch_control(format!("{key}-discount"), "Discount", on, t).on_click(
                move |_, window, cx| {
                    edit(&toggle, window, cx, |s| {
                        s.plan.discount = match s.plan.discount {
                            Some(_) => None,
                            None => Some(s.last_discount),
                        }
                    })
                },
            ),
            chips,
            note,
            t,
        )
    };
    let minimum = {
        let toggle = state.clone();
        let on = plan.minimum.is_some();
        let stepper = plan.minimum.map(|min| {
            let state = state.clone();
            stepper(
                &format!("{key}-min"),
                money_short(min),
                64.0,
                (ICON_MINUS, ICON_PLUS),
                ("Lower the minimum".into(), "Raise the minimum".into()),
                (min > MINIMUM_STEP, true),
                t,
                move |delta, window, cx| {
                    edit(&state, window, cx, |s| {
                        let next = (min + delta as i64 * MINIMUM_STEP).max(MINIMUM_STEP);
                        s.plan.minimum = Some(next);
                        s.last_minimum = next;
                    })
                },
            )
        });
        let lifted = plan.topped_up().iter().filter(|t| **t).count();
        let note = match (on, lifted) {
            (false, _) => "Top small invoices up to a floor".to_string(),
            (true, 0) => "Every invoice is already above it".to_string(),
            (true, n) => format!("Tops up {n} of {term} invoices"),
        };
        switch_line(
            pink::switch_control(format!("{key}-minimum"), "Monthly minimum", on, t).on_click(
                move |_, window, cx| {
                    edit(&toggle, window, cx, |s| {
                        s.plan.minimum = match s.plan.minimum {
                            Some(_) => None,
                            None => Some(s.last_minimum),
                        }
                    })
                },
            ),
            stepper,
            note,
            t,
        )
    };
    let adjustments = group(format!("{key}-adjustments"), "Adjustments", t)
        .child(setting("Discount", discount, t))
        .child(setting("Monthly minimum", minimum, t));

    pink::section_card(t)
        .child(header)
        .child(schedule)
        .child(adjustments)
}

/// A value between − and + (a month, a quantity, an amount). Pink has no
/// stepper: this is its Secondary control frame (pink: button.rs Secondary,
/// `fill(Quaternary)`, a `separator(true)` edge, the size's radius) holding
/// two of its icon buttons inset by the edge, the value in the control's
/// type with tabular figures between them.
#[allow(clippy::too_many_arguments)]
fn stepper(
    id: &str,
    value: impl Into<SharedString>,
    value_w: f32,
    glyphs: (&'static str, &'static str),
    names: (SharedString, SharedString),
    can: (bool, bool),
    t: &Pink,
    on_step: impl Fn(i32, &mut Window, &mut App) + 'static,
) -> Div {
    let size = ControlSize::Sm;
    let inner = size.height() - 2.0;
    let on_step = Rc::new(on_step);
    let step = |suffix: &str, glyph: &'static str, name: SharedString, enabled: bool, delta| {
        let on_step = on_step.clone();
        pink::icon_button_enabled(format!("{id}-{suffix}"), glyph, size, enabled, t)
            .w(px(inner))
            .h(px(inner))
            .rounded(px(size.radius() - 1.0))
            .aria_label(name)
            .when(enabled, |b| {
                b.on_click(move |_, window, cx| on_step(delta, window, cx))
            })
    };
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .h(px(size.height()))
        .rounded(px(size.radius()))
        .border_1()
        .border_color(t.separator(true))
        .bg(t.fill(FillTier::Quaternary))
        .child(step("less", glyphs.0, names.0, can.0, -1))
        .child(
            div()
                .w(px(value_w))
                .flex()
                .justify_center()
                .text_size(px(size.text()))
                .line_height(px(size.line_height()))
                .font_features(pink::tabular_figures())
                .text_color(t.text)
                .child(value.into()),
        )
        .child(step("more", glyphs.1, names.1, can.1, 1))
}

// ---------------------------------------------------------------------------
// Line items card
// ---------------------------------------------------------------------------

fn lines_card(
    key: &str,
    state: &Entity<PlanState>,
    s: &PlanState,
    catalog: &[Product],
    t: &Pink,
) -> Div {
    let plan = &s.plan;
    let available: Vec<Product> = catalog
        .iter()
        .filter(|p| !plan.lines.iter().any(|l| l.id == p.id))
        .cloned()
        .collect();
    let enabled = !available.is_empty();
    let open = s.menu == Some(Menu::Catalog);
    let menu = s
        .menu_phase(Menu::Catalog)
        .filter(|_| enabled)
        .map(|exit| catalog_menu(key, state, &available, exit, t));
    let add = div()
        .relative()
        .flex_none()
        .child(
            pink::button_enabled(
                format!("{key}-add"),
                ButtonKind::Secondary,
                ControlSize::Md,
                enabled,
                t,
            )
            .aria_label("Add a line item")
            .aria_expanded(open)
            .child(pink::glyph(ICON_PLUS, ControlSize::Md))
            .child("Add item")
            .when(enabled, |b| {
                let state = state.clone();
                b.on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.toggle_menu(Menu::Catalog);
                        cx.notify();
                    })
                })
            }),
        )
        .children(menu);

    let count = plan.lines.len();
    let description = match count {
        0 => "Nothing billed yet.".to_string(),
        1 => "1 item, billed on its own cadence.".to_string(),
        n => format!("{n} items, each billed on its own cadence."),
    };
    let raw_total: i64 = plan.lines.iter().map(|l| plan.line_term(l)).sum();
    let rows: Vec<AnyElement> = plan
        .lines
        .iter()
        .zip(s.keys.iter().copied())
        .enumerate()
        .map(|(ix, (line, lkey))| {
            let share = if raw_total > 0 {
                plan.line_term(line) as f64 / raw_total as f64
            } else {
                0.0
            };
            line_row(key, ix, lkey, line, share, s, state, t)
        })
        .collect();

    pink::section_card(t)
        .child(
            pink::section_header("Line items", Some(description.into()), t)
                .child(pink::row_controls().child(add)),
        )
        .children(rows)
        .when(count == 0, |card| {
            card.child(pink::card_row(false, t).child(pink::row_description(
                "Add an item from the catalog to start the schedule.",
                t,
            )))
        })
}

/// One line item: a card row (pink: usage.rs price editor rows, its provider
/// rows' 13/18 medium amount) with the cadence pop-up, quantity, amount and
/// a remove button in its controls. Lines added after the builder's rise in
/// (pink: motion.rs `fade_in`).
#[allow(clippy::too_many_arguments)]
fn line_row(
    key: &str,
    ix: usize,
    lkey: u64,
    line: &Product,
    share: f64,
    s: &PlanState,
    state: &Entity<PlanState>,
    t: &Pink,
) -> AnyElement {
    let rkey = format!("{key}-line-{lkey}");
    let menu_kind = Menu::Cadence(lkey);
    let open = s.menu == Some(menu_kind);
    let unit = match &line.unit {
        Some(unit) => format!("{} {unit}", money(line.price)),
        None => money(line.price),
    };
    let description = format!("{unit} · {} of the term", percent(share));

    let cadence = {
        let toggle = state.clone();
        let menu = s
            .menu_phase(menu_kind)
            .map(|exit| cadence_menu(&rkey, ix, line.cadence, state, exit, t));
        div()
            .relative()
            .flex_none()
            .child(
                pink::select(
                    format!("{rkey}-cadence"),
                    line.cadence.label(),
                    ControlSize::Sm,
                    open,
                    t,
                )
                .w(px(CADENCE_W))
                .aria_label(SharedString::from(format!(
                    "{} cadence: {}",
                    line.name,
                    line.cadence.label()
                )))
                .on_click(move |_, _, cx| {
                    toggle.update(cx, |s, cx| {
                        s.toggle_menu(menu_kind);
                        cx.notify();
                    })
                }),
            )
            .children(menu)
    };
    let qty = {
        let state = state.clone();
        stepper(
            &format!("{rkey}-qty"),
            line.qty.to_string(),
            32.0,
            (ICON_MINUS, ICON_PLUS),
            (
                format!("Fewer {}", line.name).into(),
                format!("More {}", line.name).into(),
            ),
            (line.qty > 1, true),
            t,
            move |delta, window, cx| {
                edit(&state, window, cx, |s| {
                    if let Some(l) = s.plan.lines.get_mut(ix) {
                        l.qty = l.qty.saturating_add_signed(delta).max(1);
                    }
                })
            },
        )
    };
    let amount = div()
        .flex_none()
        .w(px(AMOUNT_W))
        .flex()
        .justify_end()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .font_weight(FontWeight::MEDIUM)
        .font_features(pink::tabular_figures())
        .text_color(t.text)
        .child(money(line.charge()));
    let remove = {
        let state = state.clone();
        pink::icon_button(format!("{rkey}-remove"), ICON_REMOVE, ControlSize::Sm, t)
            .aria_label(SharedString::from(format!("Remove {}", line.name)))
            .tooltip(pink::text_tooltip("Remove"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, window, cx| {
                edit(&state, window, cx, |s| {
                    if ix < s.plan.lines.len() {
                        s.plan.lines.remove(ix);
                        s.keys.remove(ix);
                        s.menu = None;
                    }
                })
            })
    };

    let row = pink::card_row(false, t)
        .child(pink::row_text(
            line.name.clone(),
            Some(description.into()),
            t,
        ))
        .child(
            pink::row_controls()
                .child(cadence)
                .child(qty)
                .child(amount)
                .child(remove),
        );
    if lkey > s.seeded_through {
        motion::fade_in_rise(SharedString::from(format!("{rkey}-in")), row).into_any_element()
    } else {
        row.into_any_element()
    }
}

/// A menu row's leading check slot (pink: usage.rs environment menu
/// `check`): 14 square, holding the check on the current choice.
fn check_slot(on: bool, t: &Pink) -> Div {
    div().flex_none().size(px(14.0)).when(on, |slot| {
        slot.child(pink::line_glyph(pink::glyphs::CHECK, 14.0, t.text))
    })
}

fn cadence_menu(
    rkey: &str,
    ix: usize,
    current: Cadence,
    state: &Entity<PlanState>,
    exit: Option<f32>,
    t: &Pink,
) -> AnyElement {
    let close = state.clone();
    let rows = Cadence::ALL.map(|cadence| {
        let state = state.clone();
        pink::menu_row(format!("{rkey}-cadence-{}", cadence.label()), false, t)
            .role(Role::MenuItemRadio)
            .aria_label(cadence.label())
            .aria_toggled(toggled(cadence == current))
            .child(check_slot(cadence == current, t))
            .child(div().flex_1().min_w_0().child(cadence.label()))
            .child(
                div()
                    .flex_none()
                    .text_size(px(11.0))
                    .text_color(t.text_muted)
                    .child(cadence.every()),
            )
            .on_click(move |_, window, cx| {
                edit(&state, window, cx, |s| {
                    if let Some(l) = s.plan.lines.get_mut(ix) {
                        l.cadence = cadence;
                    }
                    s.close_menu();
                })
            })
    });
    let card = pink::popover_card(t)
        .w(px(layout::MENU_W_S))
        .flex()
        .flex_col()
        .on_mouse_down_out(move |_, _, cx| {
            close.update(cx, |s, cx| {
                s.close_menu();
                cx.notify();
            })
        })
        .children(rows);
    menu_below_end(
        SharedString::from(format!("{rkey}-cadence-menu")),
        card.into_any_element(),
        exit,
    )
}

fn catalog_menu(
    key: &str,
    state: &Entity<PlanState>,
    products: &[Product],
    exit: Option<f32>,
    t: &Pink,
) -> AnyElement {
    let close = state.clone();
    let rows = products.iter().map(|product| {
        let state = state.clone();
        let product = product.clone();
        pink::menu_row(format!("{key}-catalog-{}", product.id), false, t)
            .role(Role::MenuItem)
            .aria_label(product.name.clone())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(product.name.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(px(11.0))
                    .text_color(t.text_muted)
                    .child(format!(
                        "{} {}",
                        money(product.price),
                        product.cadence.suffix()
                    )),
            )
            .on_click(move |_, window, cx| {
                let product = product.clone();
                edit(&state, window, cx, |s| {
                    s.push_line(product);
                    s.close_menu();
                })
            })
    });
    let card = pink::popover_card(t)
        .w(px(layout::MENU_W_M))
        .flex()
        .flex_col()
        .on_mouse_down_out(move |_, _, cx| {
            close.update(cx, |s, cx| {
                s.close_menu();
                cx.notify();
            })
        })
        .child(pink::menu_heading("Catalog", t))
        .children(rows);
    menu_below_end(
        SharedString::from(format!("{key}-catalog-menu")),
        card.into_any_element(),
        exit,
    )
}

/// A menu hanging 6 px below its trigger, right-aligned to it (pink:
/// popover.rs `anchored_menu_below_end` + `menu_motion_from`): `MENU_IN`
/// with a 6 px drop while open, `MENU_OUT` under an occluder while it
/// closes, kept 8 px inside the window. Put it in a `.relative()` wrapper
/// beside the trigger.
fn menu_below_end(id: SharedString, content: AnyElement, exit: Option<f32>) -> AnyElement {
    let inner = div().occlude().pt(px(layout::POPOVER_GAP)).child(content);
    let layer = match exit {
        Some(progress) => motion::menu_out(
            SharedString::from(format!("{id}-out")),
            progress,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(id, motion::MENU_RISE_DOWN, inner).into_any_element(),
    };
    div()
        .absolute()
        .bottom_0()
        .right_0()
        .size_0()
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::TopRight)
                    .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                    .child(layer),
            )
            .priority(1),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Invoice schedule card (pink: usage.rs "Estimated cost" + "Totals")
// ---------------------------------------------------------------------------

fn schedule_card(key: &str, state: &Entity<PlanState>, s: &PlanState, t: &Pink) -> Div {
    let plan = &s.plan;
    let invoices = plan.invoices();
    let topped = plan.topped_up();
    let term = invoices.len();
    let total: i64 = invoices.iter().sum();
    let first = invoices.first().copied().unwrap_or(0);
    let peak = invoices.iter().copied().max().unwrap_or(0);
    let average = total / term.max(1) as i64;
    let savings = plan.savings();
    let lifted = topped.iter().filter(|t| **t).count();
    let hovered = s.hovered.filter(|m| *m < term);

    let mut caption = format!(
        "{term} invoices · {} to {}",
        plan.month_label(0),
        plan.month_label(term - 1)
    );
    if let Some(pct) = plan.discount {
        caption.push_str(&format!(" · {pct}% off"));
    }
    if lifted > 0 {
        caption.push_str(&format!(" · {lifted} topped up"));
    }

    // Totals: four figures, two to a line in this column.
    let figures = pink::card_row(false, t)
        .items_stretch()
        .child(metric_cell("First invoice", money(first), t))
        .child(metric_cell("Average month", money(average), t))
        .child(metric_cell("Largest invoice", money(peak), t))
        .child(metric_cell(
            "Discount",
            if savings > 0 {
                money(savings)
            } else {
                "None".into()
            },
            t,
        ));

    // Pink puts the hovered period's breakdown in the row description and
    // its total in a trailing readout. In this narrow column either would
    // wrap and push the chart away from the pointer, so the description
    // turns into the one-line readout and the tooltip carries the
    // breakdown.
    let readout = hovered.map(|m| {
        format!(
            "Invoice {} · {} · {}",
            m + 1,
            plan.month_label(m),
            money(invoices[m])
        )
    });
    let chart = pink::card_row(false, t)
        .child(pink::row_text(
            "Monthly invoices",
            Some(
                readout
                    .unwrap_or_else(|| "One point per invoice; hover for its items.".into())
                    .into(),
            ),
            t,
        ))
        .child(pink::row_block().child(invoice_chart(key, state, plan, &invoices, hovered, t)));

    pink::section_card(t)
        .child(pink::section_header(
            "Invoice schedule",
            Some(caption.into()),
            t,
        ))
        .child(
            pink::card_row(false, t).child(
                div()
                    .min_w_0()
                    .type_style(TextStyle::Display)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(money(total)),
            ),
        )
        .child(figures)
        .child(chart)
}

/// What one invoice is made of, in words: its date, each item due, the
/// discount and any top-up, then the total.
fn invoice_parts(plan: &BillingPlan, m: usize) -> Vec<String> {
    let raw: i64 = plan
        .lines
        .iter()
        .filter(|l| l.cadence.due(m))
        .map(Product::charge)
        .sum();
    let discounted = plan.discounted(raw);
    let billed = discounted.max(plan.minimum.unwrap_or(0));
    let mut parts = vec![format!("Invoice {} · {}", m + 1, plan.date_label(m))];
    parts.extend(
        plan.lines
            .iter()
            .filter(|l| l.cadence.due(m))
            .map(|l| format!("{} {}", l.name, money(l.charge()))),
    );
    if raw == 0 {
        parts.push("Nothing due".into());
    }
    if discounted < raw {
        parts.push(format!("Discount {}", money(discounted - raw)));
    }
    if billed > discounted {
        parts.push(format!("Topped up {}", money(billed - discounted)));
    }
    parts.push(format!("Total {}", money(billed)));
    parts
}

/// One figure in the Totals grid (pink: usage.rs `metric_cell`): a caption
/// label over a Title3 value on the resting plate; cells share a row and wrap
/// below 96.
fn metric_cell(label: &'static str, value: String, t: &Pink) -> Div {
    div()
        .min_w(px(96.0))
        .flex_1()
        .rounded(px(8.0))
        .border_1()
        .border_color(t.separator(false))
        .bg(t.fill(FillTier::Quaternary))
        .px(px(12.0))
        .py(px(10.0))
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(14.0))
                .text_color(t.readable_muted(0.85))
                .child(label),
        )
        .child(
            div()
                .type_style(TextStyle::Title3)
                .text_color(t.text)
                .child(value),
        )
}

/// The chart's value scale: the peak rounded up to a 1/2/5 step of half of
/// it (pink: usage.rs `usage_chart`).
fn chart_scale(peak: f64) -> f64 {
    if peak <= 0.0 {
        return 1.0;
    }
    let rough = peak / 2.0;
    let magnitude = 10_f64.powf(rough.log10().floor());
    let unit = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .find(|step| step * magnitude >= rough)
        .unwrap_or(10.0)
        * magnitude;
    (peak / unit).ceil() * unit
}

/// Monotone cubic tangents, so the curve never draws a false peak between
/// two invoices (pink: usage.rs `monotone_usage_tangents`).
fn monotone_tangents(points: &[(f32, f32)]) -> Vec<f32> {
    if points.len() < 2 {
        return vec![0.0; points.len()];
    }
    let slopes: Vec<f32> = points
        .windows(2)
        .map(|pair| {
            let dx = pair[1].0 - pair[0].0;
            if dx == 0.0 {
                0.0
            } else {
                (pair[1].1 - pair[0].1) / dx
            }
        })
        .collect();
    let mut tangents = vec![0.0; points.len()];
    tangents[0] = slopes[0];
    tangents[points.len() - 1] = slopes[slopes.len() - 1];
    for index in 1..points.len() - 1 {
        let previous = slopes[index - 1];
        let next = slopes[index];
        tangents[index] = if previous * next <= 0.0 {
            0.0
        } else {
            (previous + next) / 2.0
        };
    }
    for (index, slope) in slopes.iter().copied().enumerate() {
        if slope == 0.0 {
            tangents[index] = 0.0;
            tangents[index + 1] = 0.0;
            continue;
        }
        let a = tangents[index] / slope;
        let b = tangents[index + 1] / slope;
        let magnitude = a * a + b * b;
        if magnitude > 9.0 {
            let scale = 3.0 / magnitude.sqrt();
            tangents[index] = scale * a * slope;
            tangents[index + 1] = scale * b * slope;
        }
    }
    tangents
}

/// The per-invoice cost chart (pink: usage.rs `usage_chart`, one series in
/// pink's neutral series tone, `text`): three `border` gridlines, the area
/// at 12% under a 2 px monotone line, a 1 px `text_muted` guide at the
/// hovered invoice, a 56 px value-axis gutter and three uppercase month
/// labels, all 11/14 in `readable_muted(0.85)`. Each invoice owns an equal
/// hover cell with a tooltip of its items.
fn invoice_chart(
    key: &str,
    state: &Entity<PlanState>,
    plan: &BillingPlan,
    invoices: &[i64],
    hovered: Option<usize>,
    t: &Pink,
) -> AnyElement {
    let values: Vec<f64> = invoices.iter().map(|c| *c as f64 / 100.0).collect();
    let peak = values.iter().copied().fold(0.0_f64, f64::max);
    let scale = chart_scale(peak);
    let labels = if peak <= 0.0 {
        [String::new(), String::new(), "0".into()]
    } else {
        [scale, scale / 2.0, 0.0].map(|value| {
            if value == 0.0 {
                "0".into()
            } else {
                money_short((value * 100.0).round() as i64)
            }
        })
    };
    let series = t.text;
    let grid = t.border;
    let hover_color = t.text_muted;
    let drawing = values.clone();
    let plot = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            if width < 2.0 || height < 2.0 {
                return;
            }
            let x0 = bounds.origin.x;
            let y0 = bounds.origin.y;
            for ratio in [0.0_f32, 0.5, 1.0] {
                let mut path = PathBuilder::stroke(px(1.0));
                let y = y0 + px(ratio * height);
                path.move_to(point(x0, y));
                path.line_to(point(x0 + px(width), y));
                if let Ok(path) = path.build() {
                    window.paint_path(path, grid);
                }
            }
            if drawing.is_empty() || drawing.iter().all(|v| *v <= 0.0) {
                return;
            }
            let length = drawing.len();
            let x_at = |position: usize| {
                if length == 1 {
                    width / 2.0
                } else {
                    width * position as f32 / (length - 1) as f32
                }
            };
            let coordinates: Vec<(f32, f32)> = drawing
                .iter()
                .enumerate()
                .map(|(position, value)| {
                    let y = height - (value / scale).clamp(0.0, 1.0) as f32 * (height - 8.0);
                    (x_at(position), y)
                })
                .collect();
            let tangents = monotone_tangents(&coordinates);
            for fill in [true, false] {
                let mut path = if fill {
                    PathBuilder::fill()
                } else {
                    PathBuilder::stroke(px(2.0))
                };
                path.move_to(point(x0 + px(coordinates[0].0), y0 + px(coordinates[0].1)));
                for (index, pair) in coordinates.windows(2).enumerate() {
                    let (xa, ya) = pair[0];
                    let (xb, yb) = pair[1];
                    let dx = (xb - xa) / 3.0;
                    path.cubic_bezier_to(
                        point(x0 + px(xb), y0 + px(yb)),
                        point(x0 + px(xa + dx), y0 + px(ya + tangents[index] * dx)),
                        point(x0 + px(xb - dx), y0 + px(yb - tangents[index + 1] * dx)),
                    );
                }
                if fill {
                    let last = coordinates[coordinates.len() - 1].0;
                    path.line_to(point(x0 + px(last), y0 + px(height)));
                    path.line_to(point(x0 + px(coordinates[0].0), y0 + px(height)));
                    path.close();
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, if fill { series.opacity(0.12) } else { series });
                }
            }
            if let Some(index) = hovered.filter(|index| *index < length) {
                let x = x_at(index);
                let mut guide = PathBuilder::stroke(px(1.0));
                guide.move_to(point(x0 + px(x), y0));
                guide.line_to(point(x0 + px(x), y0 + px(height)));
                if let Ok(guide) = guide.build() {
                    window.paint_path(guide, hover_color);
                }
            }
        },
    )
    .w_full()
    .h(px(CHART_HEIGHT));

    let x_labels = if invoices.is_empty() {
        [String::new(), String::new(), String::new()]
    } else {
        let last = invoices.len() - 1;
        [0, invoices.len() / 2, last].map(|m| plan.month_label(m).to_uppercase())
    };
    let hover_cells = (0..invoices.len()).map(|m| {
        let state = state.clone();
        let tip: SharedString = invoice_parts(plan, m).join("\n").into();
        div()
            .id(SharedString::from(format!("{key}-chart-{m}")))
            .flex_1()
            .h_full()
            .on_hover(move |hovering, _, cx| {
                state.update(cx, |s, cx| {
                    if *hovering {
                        s.hovered = Some(m);
                    } else if s.hovered == Some(m) {
                        s.hovered = None;
                    }
                    cx.notify();
                })
            })
            .tooltip(move |_, cx| cx.new(|_| ChartTooltip(tip.clone())).into())
    });
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
                        .text_right()
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
                        .child(div().absolute().inset_0().flex().children(hover_cells)),
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
        .into_any_element()
}

/// The chart's hover tooltip (pink: shell.rs `SurfaceTabTooltip`): the tooltip
/// card at `max_w 380`, `px 8 / py 5`, 11/14 in `text`, one line per part.
struct ChartTooltip(SharedString);

impl Render for ChartTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        pink::tooltip_card(&t)
            .max_w(px(380.0))
            .px(px(8.0))
            .py(px(5.0))
            .text_size(px(11.0))
            .line_height(px(14.0))
            .text_color(t.text)
            .child(self.0.clone())
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        BillingSchedule::new("billing-schedule", "Northwind Ops")
            .caption("Enterprise pilot · prepared for Mara Lindqvist")
            .start(2026, 11)
            .term(12)
            .line(
                Product::new("seats", "Platform seats", 4_800)
                    .unit("per seat")
                    .qty(40),
            )
            .line(Product::new("onboarding", "Onboarding", 350_000).cadence(Cadence::Once))
            .line(Product::new("support", "Priority support", 120_000).cadence(Cadence::Quarterly))
            .discount(10)
            .catalog([
                Product::new("sandbox", "Sandbox workspace", 15_000),
                Product::new("sso", "SSO & SCIM", 30_000),
                Product::new("audit", "Audit log archive", 90_000).cadence(Cadence::Yearly),
                Product::new("residency", "EU data residency", 200_000).cadence(Cadence::Yearly),
            ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_math() {
        let plan = BillingPlan {
            start_year: 2026,
            start_month: 11,
            term: 6,
            lines: vec![
                Product::new("a", "Seats", 1_000).qty(10),
                Product::new("b", "Setup", 5_000).cadence(Cadence::Once),
                Product::new("c", "Support", 3_000).cadence(Cadence::Quarterly),
            ],
            discount: Some(10),
            minimum: Some(10_000),
        };
        let inv = plan.invoices();
        assert_eq!(inv[0], (10_000 + 5_000 + 3_000) * 9 / 10);
        assert_eq!(inv[1], 10_000); // 9,000 after discount, lifted to the minimum
        assert_eq!(inv[3], 13_000 * 9 / 10);
        assert_eq!(plan.topped_up(), [false, true, true, false, true, true]);
        assert_eq!(plan.line_term(&plan.lines[2]), 6_000);
        assert_eq!(plan.month_label(2), "Jan 2027");
        assert_eq!(plan.date_label(2), "Jan 1, 2027");
        assert_eq!(money(1_234_567), "$12,345.67");
        assert_eq!(money(-5), "−$0.05");
        assert_eq!(money_short(250_000), "$2,500");
        assert_eq!(chart_scale(5_958.0), 10_000.0);
        assert_eq!(percent(0.6923), "69.2%");
    }
}
