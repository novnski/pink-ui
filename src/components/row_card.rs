//! Row Card: the `pink-row-card` package (`components/row-card`), PINK's
//! transcript row cards as a single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The Subagent Cards story draws its cards with it; this
//! module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/row-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, Window, div, prelude::*, px};

use crate::{
    components::settings_parts as parts,
    pink::{self, ButtonKind, ControlSize, FillTier, Pink},
    story,
};

pub const NAME: &str = "Row Card";
pub const DESCRIPTION: &str = "PINK's transcript row cards as a package the app pins: a bordered card with the settings row anatomy, a 28px tile, a medium title over a quiet detail line and the actions in one trailing column, either wrapping under the text when narrow (a helper agent's card, with its type badge and status dot) or on one line (an automation's card)";

const ICON_HELIX: &str = "icons/subagent-cards/helix.svg";
const ICON_SPLIT: &str = "icons/subagent-cards/columns-2.svg";
const ICON_CLOCK: &str = "icons/automation-editor/clock-circle.svg";

/// A helper agent's card as PINK's transcript draws it (agent_cards.rs
/// `AgentCard::render`).
fn agent_card(key: &'static str, t: &Pink) -> impl IntoElement {
    let shape = Layout::Wrapping;
    card(
        shape,
        &CardStyle {
            border: t.hairline(0.07),
            background: t.ink(0.03),
        },
    )
    .id(key)
    .role(Role::Group)
    .child(parts::row_tile(
        &parts::TileStyle {
            border: t.hairline(0.07),
            background: t.fill(FillTier::Quaternary),
            icon: t.content_muted,
        },
        ICON_HELIX,
    ))
    .child(
        text_column(shape)
            .child(
                title_line()
                    .child(title(t.text, "Audit webhook retries"))
                    .child(type_badge(
                        &BadgeStyle {
                            background: t.ink(0.06),
                            text: t.content_muted,
                        },
                        "Fresh side chat",
                    )),
            )
            .child(
                meta_line(t.content_muted)
                    .child(status(t.accent, None, "Running"))
                    .child(fact("1m 12s"))
                    .child(fact("Helix 4 Pro High"))
                    .child(preview(
                        t.readable_muted(0.8),
                        "List every webhook handler that is not idempotent.",
                    )),
            ),
    )
    .child(
        actions(shape)
            .child(
                pink::icon_button(format!("{key}-split"), ICON_SPLIT, ControlSize::Sm, t)
                    .aria_label("Show agent chat in split"),
            )
            .child(
                pink::button(
                    format!("{key}-open"),
                    ButtonKind::Secondary,
                    ControlSize::Sm,
                    t,
                )
                .child("Open chat"),
            ),
    )
}

/// An automation's card as PINK's transcript draws it (automations.rs
/// `render_card`).
fn automation_card(key: &'static str, t: &Pink) -> impl IntoElement {
    let shape = Layout::Inline;
    card(
        shape,
        &CardStyle {
            border: t.separator(false),
            background: t.fill(FillTier::Quaternary),
        },
    )
    .id(key)
    .role(Role::Group)
    .child(parts::row_tile(
        &parts::TileStyle {
            border: t.border,
            background: t.fill(FillTier::Quaternary),
            icon: t.text_muted,
        },
        ICON_CLOCK,
    ))
    .child(
        text_column(shape)
            .child(title(t.text, "Morning dependency check"))
            .child(detail(
                t.readable_muted(0.85),
                "Weekdays at 9:00 in fieldnotes, next run tomorrow",
            )),
    )
    .child(
        actions(shape)
            .child(
                pink::button(format!("{key}-edit"), ButtonKind::Ghost, ControlSize::Sm, t)
                    .child("Edit"),
            )
            .child(
                pink::button(
                    format!("{key}-open"),
                    ButtonKind::Secondary,
                    ControlSize::Sm,
                    t,
                )
                .child("Open"),
            ),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .w(px(560.0))
                    .child(agent_card("story-row-card-agent", &t)),
            )
            .child(
                div()
                    .w(px(340.0))
                    .child(agent_card("story-row-card-agent-narrow", &t)),
            )
            .child(
                div()
                    .w(px(560.0))
                    .child(automation_card("story-row-card-automation", &t)),
            )
    })
}
