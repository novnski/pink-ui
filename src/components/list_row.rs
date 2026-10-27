//! List Row: the `pink-list-row` package (`components/list-row`), PINK's
//! sidebar row and status indicator as one single-GPUI package the app pins
//! (novnski/pink#173, decisions 54 to 56). The gallery compiles the
//! package's own source here, unchanged, against its GPUI; PINK compiles the
//! same file against its patched one. This module adds only the story.
//!
//! Rows are 32 pt for one line and 52 for two, scaled with the Interface
//! size, with the title on one column in every scope and the time ending on
//! the other; hover swaps the time for the actions in a fixed corner, so the
//! title never moves, and there is one selected style. The status indicator
//! keeps running, waiting for a reply or an approval, failed and unread
//! apart, each with a spoken name.

#[allow(dead_code)]
mod package {
    include!("../../components/list-row/src/lib.rs");
}
pub use package::*;

use gpui::{AnyElement, AnyView, App, IntoElement, SharedString, Window, div, prelude::*, px, svg};

use crate::{
    pink::{self, Pink, chrome, layout, loaders, radius},
    story,
};

pub const NAME: &str = "List Row";
pub const DESCRIPTION: &str = "PINK's sidebar row as a package the app pins: one title column in every scope, the time on the other, a fixed corner where hover actions replace the time, one selected style, and a status indicator that keeps running, waiting, failed and unread apart";

const ICON_SHIELD: &str = "icons/sidebar/shield-warning.svg";
const ICON_QUESTION: &str = "icons/sidebar/chat-round-question-mark.svg";
const ICON_DANGER: &str = "icons/sidebar/danger-circle.svg";
const ICON_BRANCH: &str = "icons/sidebar/git-branch.svg";
const ICON_PIN: &str = "icons/sidebar/pin.svg";
const ICON_ARCHIVE: &str = "icons/sidebar/archive-minimalistic.svg";
const ICON_CLAUDE: &str = "icons/sidebar/claude-mark.svg";

/// The kit's tokens as the package's host tokens: what Pink's sidebar paints
/// (`render_chat_row`), floored by the kit for contrast.
pub fn row_style(t: &Pink, scale: f32) -> RowStyle {
    let surface = t.chrome_surface();
    let tone = |c: gpui::Hsla| t.readable_tone(c, 1.0);
    RowStyle {
        scale,
        radius: radius::MD,
        title: t.text.opacity(0.8),
        title_selected: t.text,
        secondary: t.readable_muted(0.5),
        meta: t.readable_muted(0.55),
        rest_bg: t.wash(0.0),
        hover_bg: t.sidebar_hover_bg(),
        selected_bg: t.glass_selected_bg(),
        corner_width: 52.0 * scale,
        running: t.busy,
        waiting: t.readable_on(
            t.warning,
            pink::flatten(t.warning.opacity(0.12), surface),
            pink::TEXT_CONTRAST_TARGET,
        ),
        failed: tone(t.danger),
        unread: t.success,
        pill_backing: 0.12,
    }
}

fn glyph(path: &'static str, size: f32, color: gpui::Hsla) -> AnyElement {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
        .into_any_element()
}

/// The hover actions a host might put in the corner.
fn actions(t: &Pink, scale: f32) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .children([ICON_PIN, ICON_ARCHIVE].into_iter().map(|icon| {
            div()
                .size(px(chrome::MIN_HIT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(chrome::RADIUS_S))
                .child(glyph(icon, chrome::ICON_S * scale, t.text_muted))
        }))
        .into_any_element()
}

fn secondary(branch: &'static str, t: &Pink, scale: f32) -> AnyElement {
    let tone = t.readable_muted(0.5);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0 * scale))
        .child(glyph(ICON_CLAUDE, 11.0 * scale, tone))
        .child(glyph(ICON_BRANCH, 11.0 * scale, tone))
        .child(branch)
        .into_any_element()
}

struct Sample {
    title: &'static str,
    branch: &'static str,
    time: &'static str,
    status: Option<Signal>,
}

const SAMPLES: [Sample; 6] = [
    Sample {
        title: "Backfill invoice PDFs",
        branch: "invoices/backfill",
        time: "now",
        status: Some(Signal::Running),
    },
    Sample {
        title: "Retire expired coupon codes",
        branch: "coupons/retire",
        time: "2m",
        status: Some(Signal::NeedsApproval),
    },
    Sample {
        title: "Which VAT rate applies to Norway?",
        branch: "tax/norway",
        time: "7m",
        status: Some(Signal::WaitingForInput),
    },
    Sample {
        title: "Upgrade the Postgres driver",
        branch: "deps/pg",
        time: "30m",
        status: Some(Signal::Failed),
    },
    Sample {
        title: "Retry failed Stripe webhooks",
        branch: "webhooks/retry",
        time: "15m",
        status: Some(Signal::Unread),
    },
    Sample {
        title: "Draft dunning email copy",
        branch: "copy/dunning",
        time: "5h",
        status: None,
    },
];

fn rows(scope: &'static str, two_lines: bool, t: &Pink, scale: f32) -> Vec<AnyElement> {
    let style = row_style(t, scale);
    SAMPLES
        .iter()
        .enumerate()
        .map(|(ix, sample)| {
            let status_glyph = sample.status.and_then(|signal| match signal {
                Signal::Running => Some(
                    loaders::mini_glyph_spinner(format!("{scope}-spin-{ix}"), 2.0, t.glyph)
                        .into_any_element(),
                ),
                Signal::NeedsApproval => Some(glyph(ICON_SHIELD, 11.0 * scale, style.waiting)),
                Signal::WaitingForInput => Some(glyph(ICON_QUESTION, 11.0 * scale, style.waiting)),
                Signal::Failed => Some(glyph(ICON_DANGER, 11.0 * scale, style.failed)),
                Signal::Unread | Signal::Queued => None,
            });
            // Row 0 is selected and row 2 hovered, so both styles show.
            let row = ListRow {
                title: sample.title.into(),
                secondary: two_lines.then(|| secondary(sample.branch, t, scale)),
                leading: None,
                time: Some(SharedString::from(sample.time)),
                status: sample.status,
                status_glyph,
                chips: Vec::new(),
                actions: Some(actions(t, scale)),
                selected: ix == 0,
                hovered: ix == 2,
                ..ListRow::new(sample.title)
            };
            let id = format!("{scope}-row-{ix}");
            // The host's semantics: a selectable button named with its
            // signal.
            let (name, selected) = (row.accessible_name(), row.selected);
            list_row(id.clone(), row, &style)
                .role(gpui::Role::Button)
                .aria_label(name)
                .aria_selected(selected)
                .tab_index(0)
                .focus_visible({
                    let ring = t.focus_ring();
                    move |s| pink::focus_ring(s, ring)
                })
                .tooltip(pink::text_tooltip(sample.title))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
                .into_any_element()
        })
        .collect()
}

/// One sidebar column: a scope label, then the rows on the shell surface
/// with Pink's 8 pt gutter.
fn column(label: &'static str, list: Vec<AnyElement>, t: &Pink, scale: f32) -> impl IntoElement {
    div()
        .w(px(288.0))
        .flex()
        .flex_col()
        .px(px(8.0))
        .py(px(8.0))
        .rounded(px(radius::PANEL))
        .bg(t.chrome_surface())
        .font_family(pink::SANS)
        .child(
            div()
                .px(px(ROW_INSET))
                .h(px(28.0 * scale))
                .flex()
                .items_center()
                .text_size(px(11.0 * scale))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(t.readable_faint(0.9))
                .child(label),
        )
        .children(list)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let scale = pink::ui_scale(cx);
        div()
            .flex()
            .flex_row()
            .gap(px(24.0))
            .child(column(
                "All projects",
                rows("all", false, &t, scale),
                &t,
                scale,
            ))
            .child(column(
                "billing-api",
                rows("project", true, &t, scale),
                &t,
                scale,
            ))
    })
}
