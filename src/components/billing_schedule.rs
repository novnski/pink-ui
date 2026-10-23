//! Billing Schedule – compose a billing plan and see every invoice it makes.
//!
//! The left side builds the plan: a start month, a term, line items with
//! their own cadence (monthly, quarterly, yearly or once) and quantity, and
//! two optional adjustments – a percentage discount and a monthly minimum
//! that tops small invoices up. The right side is the schedule it produces:
//! one bar per invoice (hover for the amount), the first invoice, the
//! average month and the term total, all gliding to their new values.
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

use gpui::{
    AnyElement, AnyView, App, ElementId, Entity, FontWeight, Hsla, IntoElement, SharedString,
    Window, deferred, div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, MotionSpec},
    primitives::shadow,
    story,
};

pub const NAME: &str = "Billing Schedule";
pub const DESCRIPTION: &str = "Compose a billing plan from line items with their own cadence, a discount and a monthly minimum, and watch the invoice schedule and totals update live";

const ICON_PREV: &str = "icons/billing-schedule/chevron-left.svg";
const ICON_NEXT: &str = "icons/billing-schedule/chevron-right.svg";
const ICON_PLUS: &str = "icons/billing-schedule/plus.svg";
const ICON_MINUS: &str = "icons/billing-schedule/minus.svg";
const ICON_REMOVE: &str = "icons/billing-schedule/x.svg";
const ICON_REPEAT: &str = "icons/billing-schedule/repeat.svg";
const ICON_CALENDAR: &str = "icons/billing-schedule/calendar.svg";
const ICON_TAG: &str = "icons/billing-schedule/tag.svg";
const ICON_FLOOR: &str = "icons/billing-schedule/floor.svg";

const TERMS: [usize; 4] = [3, 6, 12, 24];
const DISCOUNTS: [u32; 4] = [5, 10, 15, 20];
const MINIMUM_STEP: i64 = 50_000;
const CHART_H: f32 = 104.;
const SUMMARY_W: f32 = 268.;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// Totals count toward their new value.
const COUNT: MotionSpec = MotionSpec::new(520, motion::EASE_OUT_QUINT);

/// How often a line item is charged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cadence {
    Monthly,
    Quarterly,
    Yearly,
    Once,
}

impl Cadence {
    fn label(self) -> &'static str {
        match self {
            Cadence::Monthly => "Monthly",
            Cadence::Quarterly => "Quarterly",
            Cadence::Yearly => "Yearly",
            Cadence::Once => "Once",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Cadence::Monthly => "/ mo",
            Cadence::Quarterly => "/ qtr",
            Cadence::Yearly => "/ yr",
            Cadence::Once => "once",
        }
    }

    fn next(self) -> Self {
        match self {
            Cadence::Monthly => Cadence::Quarterly,
            Cadence::Quarterly => Cadence::Yearly,
            Cadence::Yearly => Cadence::Once,
            Cadence::Once => Cadence::Monthly,
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

    fn monogram(&self) -> String {
        let words: Vec<&str> = self
            .name
            .split_whitespace()
            .filter(|w| w.starts_with(char::is_alphanumeric))
            .collect();
        let letters: String = if words.len() >= 2 {
            words
                .iter()
                .take(2)
                .filter_map(|w| w.chars().next())
                .collect()
        } else {
            self.name.chars().take(2).collect()
        };
        letters.to_uppercase()
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
    /// One amount (cents) per invoice, before any minimum.
    fn raw(&self) -> Vec<i64> {
        (0..self.term.max(1))
            .map(|m| {
                self.lines
                    .iter()
                    .filter(|l| l.cadence.due(m))
                    .map(|l| l.price * l.qty as i64)
                    .sum()
            })
            .collect()
    }

    /// One amount per invoice after the discount and the minimum.
    pub fn invoices(&self) -> Vec<i64> {
        self.raw()
            .into_iter()
            .map(|raw| {
                let discounted = raw * (100 - self.discount.unwrap_or(0) as i64) / 100;
                discounted.max(self.minimum.unwrap_or(0))
            })
            .collect()
    }

    /// Months the minimum lifted.
    fn topped_up(&self) -> Vec<bool> {
        self.raw()
            .into_iter()
            .map(|raw| {
                let discounted = raw * (100 - self.discount.unwrap_or(0) as i64) / 100;
                self.minimum.is_some_and(|min| discounted < min)
            })
            .collect()
    }

    pub fn total(&self) -> i64 {
        self.invoices().iter().sum()
    }

    /// What the discount takes off over the term.
    pub fn savings(&self) -> i64 {
        let raw: i64 = self.raw().iter().sum();
        raw - raw * (100 - self.discount.unwrap_or(0) as i64) / 100
    }

    fn month_label(&self, offset: usize) -> String {
        let index = self.start_year * 12 + self.start_month as i32 - 1 + offset as i32;
        format!(
            "{} {}",
            MONTHS[index.rem_euclid(12) as usize],
            index.div_euclid(12)
        )
    }
}

/// `$12,345.67`.
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

type ChangeHandler = Rc<dyn Fn(&BillingPlan, &mut Window, &mut App)>;

struct PlanState {
    seeded: bool,
    plan: BillingPlan,
    keys: Vec<u64>,
    next_key: u64,
    last_discount: u32,
    last_minimum: i64,
    menu_open: bool,
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
            last_discount: 10,
            last_minimum: 250_000,
            menu_open: false,
            hovered: None,
            on_change: None,
        }
    }

    fn push_line(&mut self, product: Product) {
        self.plan.lines.push(product);
        self.keys.push(self.next_key);
        self.next_key += 1;
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
        let t = Luna::get(cx);
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
            }
        });

        let composer = composer(
            &key,
            &state,
            &self.customer,
            &self.caption,
            &self.catalog,
            &t,
            cx,
        );
        let summary = summary(&key, &state, &t, cx);

        luna::card(&t)
            .id(self.id.clone())
            .w(px(800.))
            .flex()
            .font_family(luna::SANS)
            .text_color(t.text)
            .text_size(px(text::BODY))
            .child(composer)
            .child(summary)
            .into_any_element()
    }
}

fn label(text_: &'static str, t: &Luna) -> gpui::Div {
    div()
        .text_size(px(text::FOOTNOTE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text_muted)
        .child(text_)
}

fn composer(
    key: &str,
    state: &Entity<PlanState>,
    customer: &SharedString,
    caption: &SharedString,
    catalog: &[Product],
    t: &Luna,
    cx: &App,
) -> impl IntoElement {
    let s = state.read(cx);
    let plan = &s.plan;

    let initials: String = customer
        .split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .collect::<String>()
        .to_uppercase();
    let header = div()
        .flex()
        .items_center()
        .gap(px(space::S12))
        .child(
            div()
                .size(px(36.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Tertiary))
                .text_size(px(text::FOOTNOTE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text_muted)
                .child(initials),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .child(
                    div()
                        .text_size(px(text::CALLOUT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(customer.clone()),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child(caption.clone()),
                ),
        )
        .child(luna::badge("Draft", Tone::Warning, t));

    // Start month stepper and term chips.
    let shift = |delta: i32| {
        let state = state.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
            edit(&state, window, cx, |s| {
                let index = s.plan.start_year * 12 + s.plan.start_month as i32 - 1 + delta;
                s.plan.start_year = index.div_euclid(12);
                s.plan.start_month = index.rem_euclid(12) as u32 + 1;
            })
        }
    };
    let start = div()
        .flex()
        .items_center()
        .h(px(28.))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(t.border)
        .bg(t.input_bg)
        .child(
            luna::icon_button(format!("{key}-start-prev"), ICON_PREV, ControlSize::Sm, t)
                .on_click(shift(-1)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::S6))
                .px(px(space::S4))
                .w(px(92.))
                .justify_center()
                .text_size(px(text::FOOTNOTE))
                .font_weight(FontWeight::MEDIUM)
                .child(
                    svg()
                        .path(ICON_CALENDAR)
                        .size(px(13.))
                        .text_color(t.text_faint),
                )
                .child(plan.month_label(0)),
        )
        .child(
            luna::icon_button(format!("{key}-start-next"), ICON_NEXT, ControlSize::Sm, t)
                .on_click(shift(1)),
        );
    let terms = div()
        .flex()
        .gap(px(space::S4))
        .children(TERMS.map(|months| {
            let state = state.clone();
            luna::chip(
                format!("{key}-term-{months}"),
                plan.term == months,
                ControlSize::Md,
                t,
            )
            .child(format!("{months} mo"))
            .on_click(move |_, window, cx| edit(&state, window, cx, |s| s.plan.term = months))
        }));
    let controls = div()
        .flex()
        .items_end()
        .gap(px(space::S20))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::S6))
                .child(label("Starts", t))
                .child(start),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::S6))
                .child(label("Term", t))
                .child(terms),
        );

    // Line items.
    let lines: Vec<_> = plan
        .lines
        .iter()
        .zip(s.keys.iter().copied())
        .enumerate()
        .map(|(ix, (line, lkey))| line_row(key, ix, lkey, line, state, t))
        .collect();
    let available: Vec<Product> = catalog
        .iter()
        .filter(|p| !plan.lines.iter().any(|l| l.id == p.id))
        .cloned()
        .collect();
    let add = {
        let state = state.clone();
        let open = s.menu_open;
        let disabled = available.is_empty();
        let menu = (open && !disabled).then(|| add_menu(key, state.clone(), &available, t));
        div()
            .relative()
            .child(
                luna::button(format!("{key}-add"), ButtonKind::Ghost, ControlSize::Md, t)
                    .when(disabled, |b| b.opacity(0.5))
                    .child(luna::glyph(ICON_PLUS, ControlSize::Md).text_color(t.text_muted))
                    .child("Add item")
                    .on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.menu_open = !s.menu_open && !disabled;
                            cx.notify();
                        })
                    }),
            )
            .children(menu)
    };

    // Adjustments: discount and minimum.
    let discount_on = plan.discount.is_some();
    let minimum_on = plan.minimum.is_some();
    let discount = {
        let toggle = state.clone();
        let chips = discount_on.then(|| {
            div()
                .flex()
                .gap(px(space::S4))
                .children(DISCOUNTS.map(|pct| {
                    let state = state.clone();
                    luna::chip(
                        format!("{key}-pct-{pct}"),
                        plan.discount == Some(pct),
                        ControlSize::Sm,
                        t,
                    )
                    .child(format!("{pct}%"))
                    .on_click(move |_, window, cx| {
                        edit(&state, window, cx, |s| {
                            s.plan.discount = Some(pct);
                            s.last_discount = pct;
                        })
                    })
                }))
        });
        adjustment(
            &format!("{key}-discount"),
            ICON_TAG,
            "Discount",
            "Taken off every invoice",
            discount_on,
            t,
            move |window, cx| {
                edit(&toggle, window, cx, |s| {
                    s.plan.discount = match s.plan.discount {
                        Some(_) => None,
                        None => Some(s.last_discount),
                    }
                })
            },
        )
        .children(chips)
    };
    let minimum = {
        let toggle = state.clone();
        let stepper = plan.minimum.map(|min| {
            let down = state.clone();
            let up = state.clone();
            div()
                .flex()
                .items_center()
                .h(px(24.))
                .rounded(px(radius::SM))
                .border_1()
                .border_color(t.border)
                .bg(t.input_bg)
                .child(
                    luna::icon_button(format!("{key}-min-down"), ICON_MINUS, ControlSize::Sm, t)
                        .size(px(22.))
                        .on_click(move |_, window, cx| {
                            edit(&down, window, cx, |s| {
                                let next = (min - MINIMUM_STEP).max(MINIMUM_STEP);
                                s.plan.minimum = Some(next);
                                s.last_minimum = next;
                            })
                        }),
                )
                .child(
                    div()
                        .w(px(72.))
                        .flex()
                        .justify_center()
                        .text_size(px(text::FOOTNOTE))
                        .font_family(luna::MONO)
                        .child(money(min).trim_end_matches(".00").to_string()),
                )
                .child(
                    luna::icon_button(format!("{key}-min-up"), ICON_PLUS, ControlSize::Sm, t)
                        .size(px(22.))
                        .on_click(move |_, window, cx| {
                            edit(&up, window, cx, |s| {
                                let next = min + MINIMUM_STEP;
                                s.plan.minimum = Some(next);
                                s.last_minimum = next;
                            })
                        }),
                )
        });
        adjustment(
            &format!("{key}-minimum"),
            ICON_FLOOR,
            "Monthly minimum",
            "Small invoices are topped up",
            minimum_on,
            t,
            move |window, cx| {
                edit(&toggle, window, cx, |s| {
                    s.plan.minimum = match s.plan.minimum {
                        Some(_) => None,
                        None => Some(s.last_minimum),
                    }
                })
            },
        )
        .children(stepper)
    };

    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(space::S16))
        .p(px(space::S20))
        .child(header)
        .child(controls)
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(space::S4))
                        .child(label("Line items", t))
                        .child(add),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .rounded(px(radius::LG))
                        .border_1()
                        .border_color(t.border)
                        .children(lines),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::S8))
                .child(discount)
                .child(minimum),
        )
}

fn line_row(
    key: &str,
    ix: usize,
    lkey: u64,
    line: &Product,
    state: &Entity<PlanState>,
    t: &Luna,
) -> impl IntoElement {
    let rkey = format!("{key}-line-{lkey}");
    let hover = motion::hover_t(&rkey);
    let cycle = state.clone();
    let less = state.clone();
    let more = state.clone();
    let remove = state.clone();
    let qty = line.qty;
    let unit_line = match &line.unit {
        Some(unit) => format!("{} {unit}", money(line.price)),
        None => money(line.price),
    };
    let cadence = line.cadence;
    let cadence_key = format!("{rkey}-cadence");

    motion::rise_in(
        SharedString::from(format!("{rkey}-in")),
        motion::ROW_IN,
        6.,
        div()
            .id(SharedString::from(rkey.clone()))
            .flex()
            .items_center()
            .gap(px(space::S8))
            .h(px(52.))
            .pl(px(space::S10))
            .pr(px(space::S4))
            .when(ix > 0, |d| d.border_t_1().border_color(t.border))
            .hover_fill(
                rkey.clone(),
                t.fill(FillTier::Quaternary).opacity(0.),
                t.fill(FillTier::Quaternary),
            )
            .child(
                div()
                    .size(px(30.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::SM))
                    .bg(t.fill(FillTier::Tertiary))
                    .text_size(px(text::CAPTION))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text_muted)
                    .child(line.monogram()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(line.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(unit_line),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(cadence_key.clone()))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .h(px(24.))
                    .w(px(84.))
                    .px(px(space::S8))
                    .rounded(px(radius::SM))
                    .cursor_pointer()
                    .text_size(px(text::FOOTNOTE))
                    .hover_fill_fg(
                        cadence_key,
                        t.fill(FillTier::Quaternary),
                        t.fill(FillTier::Secondary),
                        t.text_muted,
                        t.text,
                    )
                    .child(
                        svg()
                            .path(ICON_REPEAT)
                            .size(px(12.))
                            .text_color(t.text_faint),
                    )
                    .child(cadence.label())
                    .on_click(move |_, window, cx| {
                        edit(&cycle, window, cx, |s| {
                            if let Some(l) = s.plan.lines.get_mut(ix) {
                                l.cadence = l.cadence.next();
                            }
                        })
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(24.))
                    .rounded(px(radius::SM))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.input_bg)
                    .child(
                        luna::icon_button(format!("{rkey}-less"), ICON_MINUS, ControlSize::Sm, t)
                            .size(px(22.))
                            .on_click(move |_, window, cx| {
                                edit(&less, window, cx, |s| {
                                    if let Some(l) = s.plan.lines.get_mut(ix) {
                                        l.qty = l.qty.saturating_sub(1).max(1);
                                    }
                                })
                            }),
                    )
                    .child(
                        div()
                            .w(px(28.))
                            .flex()
                            .justify_center()
                            .text_size(px(text::FOOTNOTE))
                            .font_family(luna::MONO)
                            .child(qty.to_string()),
                    )
                    .child(
                        luna::icon_button(format!("{rkey}-more"), ICON_PLUS, ControlSize::Sm, t)
                            .size(px(22.))
                            .on_click(move |_, window, cx| {
                                edit(&more, window, cx, |s| {
                                    if let Some(l) = s.plan.lines.get_mut(ix) {
                                        l.qty += 1;
                                    }
                                })
                            }),
                    ),
            )
            .child(
                div()
                    .w(px(92.))
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(money(line.price * qty as i64)),
                    )
                    .child(
                        div()
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_faint)
                            .child(cadence.suffix()),
                    ),
            )
            .child(
                luna::icon_button(format!("{rkey}-remove"), ICON_REMOVE, ControlSize::Sm, t)
                    .opacity(0.25 + 0.75 * hover)
                    .on_click(move |_, window, cx| {
                        edit(&remove, window, cx, |s| {
                            if ix < s.plan.lines.len() {
                                s.plan.lines.remove(ix);
                                s.keys.remove(ix);
                            }
                        })
                    }),
            ),
    )
}

fn add_menu(
    key: &str,
    state: Entity<PlanState>,
    products: &[Product],
    t: &Luna,
) -> impl IntoElement {
    let close = state.clone();
    let items = products.iter().enumerate().map(|(ix, product)| {
        let state = state.clone();
        let product = product.clone();
        let hover = format!("{key}-catalog-{ix}");
        div()
            .id(SharedString::from(hover.clone()))
            .flex()
            .items_center()
            .gap(px(space::S10))
            .h(px(40.))
            .px(px(space::S8))
            .rounded(px(radius::SM))
            .cursor_pointer()
            .hover_fill(hover, t.element_hover.opacity(0.), t.element_hover)
            .child(
                div()
                    .size(px(24.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::XS))
                    .bg(t.fill(FillTier::Tertiary))
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text_muted)
                    .child(product.monogram()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(product.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_faint)
                            .child(format!(
                                "{} {}",
                                money(product.price),
                                product.cadence.suffix()
                            )),
                    ),
            )
            .on_click(move |_, window, cx| {
                let product = product.clone();
                edit(&state, window, cx, |s| {
                    s.push_line(product);
                    s.menu_open = false;
                })
            })
    });
    deferred(
        div()
            .absolute()
            .top(px(32.))
            .right_0()
            .child(motion::pop_in(
                SharedString::from(format!("{key}-menu")),
                luna::overlay(t)
                    .id(SharedString::from(format!("{key}-menu-list")))
                    .w(px(248.))
                    .p(px(space::S4))
                    .flex()
                    .flex_col()
                    .on_mouse_down_out(move |_, _, cx| {
                        close.update(cx, |s, cx| {
                            s.menu_open = false;
                            cx.notify();
                        })
                    })
                    .child(
                        div()
                            .px(px(space::S8))
                            .pt(px(space::S4))
                            .pb(px(space::S6))
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_faint)
                            .child("Catalog"),
                    )
                    .children(items),
            )),
    )
    .with_priority(1)
}

/// A switch row for an optional adjustment; its controls follow when it's on.
fn adjustment(
    key: &str,
    icon: &'static str,
    title: &'static str,
    hint: &'static str,
    on: bool,
    t: &Luna,
    toggle: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(key.to_string()))
        .flex()
        .items_center()
        .gap(px(space::S10))
        .h(px(36.))
        .child(
            div()
                .id(SharedString::from(format!("{key}-toggle")))
                .flex()
                .items_center()
                .gap(px(space::S10))
                .flex_1()
                .cursor_pointer()
                .on_click(move |_, window, cx| toggle(window, cx))
                .child(switch(key, on, t))
                .child(svg().path(icon).size(px(14.)).text_color(if on {
                    t.accent
                } else {
                    t.text_faint
                }))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(space::S6))
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .child(
                            div()
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_faint)
                                .child(hint),
                        ),
                ),
        )
}

fn summary(key: &str, state: &Entity<PlanState>, t: &Luna, cx: &App) -> impl IntoElement {
    let s = state.read(cx);
    let plan = &s.plan;
    let invoices = plan.invoices();
    let topped = plan.topped_up();
    let term = invoices.len();
    let peak = invoices.iter().copied().max().unwrap_or(0).max(1) as f32;
    let first = invoices.first().copied().unwrap_or(0);
    let total: i64 = invoices.iter().sum();
    let average = total / term.max(1) as i64;
    let savings = plan.savings();

    // Bars: equal widths across the chart, heights gliding to scale.
    let chart_w = SUMMARY_W - 2. * space::S20;
    let gap = if term > 12 { 2. } else { 4. };
    let bar_w = ((chart_w - gap * (term as f32 - 1.)) / term as f32).max(2.);
    let hovered = s.hovered.filter(|h| *h < term);
    let bars = invoices.iter().enumerate().map(|(m, amount)| {
        let h = motion::follow(
            &format!("{key}-bar-{m}"),
            (*amount as f32 / peak) * (CHART_H - 4.) + 2.,
            MotionSpec::new(480, motion::EASE_OUT_QUINT).with_delay((m as u64 * 12).min(200)),
        );
        let color: Hsla = if topped.get(m).copied().unwrap_or(false) {
            t.warning
        } else if m == 0 {
            t.accent_strong
        } else {
            t.accent
        };
        let dim = hovered.is_some_and(|hm| hm != m);
        let state = state.clone();
        div()
            .id(SharedString::from(format!("{key}-col-{m}")))
            .w(px(bar_w))
            .h_full()
            .flex()
            .flex_col()
            .justify_end()
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
            .child(
                div()
                    .w_full()
                    .h(px(h.max(2.)))
                    .rounded_t(px(radius::XS.min(bar_w / 2.)))
                    .rounded_b(px(1.))
                    .bg(color.opacity(if dim {
                        0.35
                    } else if m == 0 {
                        1.
                    } else {
                        0.7
                    })),
            )
    });

    let readout: (String, i64) = match hovered {
        Some(m) => (
            format!("Invoice {} · {}", m + 1, plan.month_label(m)),
            invoices[m],
        ),
        None => ("Largest invoice".into(), peak as i64),
    };

    let counter = |name: &str, cents: i64| {
        let shown = motion::follow(&format!("{key}-count-{name}"), cents as f32 / 100., COUNT);
        let settled = (shown - cents as f32 / 100.).abs() < 0.01;
        if settled {
            money(cents)
        } else {
            money((shown * 100.).round() as i64)
        }
    };
    let row = |name: &'static str, value: String, color: Hsla| {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(26.))
            .child(div().text_color(t.text_muted).child(name))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color)
                    .child(value),
            )
    };

    div()
        .w(px(SUMMARY_W))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(space::S16))
        .p(px(space::S20))
        .bg(t.fill(FillTier::Quaternary))
        .border_l_1()
        .border_color(t.border)
        .rounded_r(px(radius::LG))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(label("Invoice schedule", t))
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_faint)
                        .child(format!(
                            "{} – {} · {term} invoices",
                            plan.month_label(0),
                            plan.month_label(term - 1)
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(space::S8))
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(text::CAPTION))
                                .text_color(t.text_faint)
                                .child(readout.0),
                        )
                        .child(
                            div()
                                .text_size(px(text::FOOTNOTE))
                                .font_weight(FontWeight::MEDIUM)
                                .font_family(luna::MONO)
                                .child(money(readout.1)),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("{key}-chart")))
                        .h(px(CHART_H))
                        .flex()
                        .items_end()
                        .gap(px(gap))
                        .border_b_1()
                        .border_color(t.border_strong)
                        .children(bars),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(text::CAPTION))
                        .text_color(t.text_faint)
                        .child(plan.month_label(0))
                        .child(plan.month_label(term - 1)),
                )
                .when(topped.iter().any(|t| *t), |d| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S6))
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_muted)
                            .child(
                                div()
                                    .size(px(8.))
                                    .rounded(px(2.))
                                    .bg(t.warning.opacity(0.7)),
                            )
                            .child("Topped up to the monthly minimum"),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .text_size(px(text::BODY))
                .child(row("First invoice", counter("first", first), t.text))
                .child(row("Average month", counter("average", average), t.text))
                .when(savings > 0, |d| {
                    d.child(row(
                        "Discount",
                        format!("−{}", counter("savings", savings)),
                        t.success,
                    ))
                })
                .child(div().my(px(space::S8)).child(luna::divider(t)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().font_weight(FontWeight::MEDIUM).child("Term total"))
                                .child(
                                    div()
                                        .text_size(px(text::CAPTION))
                                        .text_color(t.text_faint)
                                        .child(format!("over {term} months")),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(text::LARGE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.text)
                                .child(counter("total", total)),
                        ),
                ),
        )
}

/// An on/off switch whose knob glides.
fn switch(key: &str, on: bool, t: &Luna) -> impl IntoElement {
    let k = motion::follow(
        &format!("{key}-knob"),
        if on { 1. } else { 0. },
        MotionSpec::new(240, motion::EASE_OUT_QUINT),
    );
    div()
        .w(px(30.))
        .h(px(18.))
        .flex_none()
        .p(px(2.))
        .rounded_full()
        .bg(motion::mix(t.surface_raised_hover, t.accent_strong, k))
        .child(
            div()
                .size(px(14.))
                .ml(px(12. * k))
                .rounded_full()
                .bg(gpui::white())
                .shadow(vec![shadow(0., 1., 2., 0., gpui::black().opacity(0.22))]),
        )
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
        assert_eq!(plan.month_label(2), "Jan 2027");
        assert_eq!(money(1_234_567), "$12,345.67");
        assert_eq!(money(-5), "−$0.05");
    }
}
