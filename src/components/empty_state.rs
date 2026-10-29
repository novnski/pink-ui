//! Empty State: the `pink-empty-state` package (`components/empty-state`),
//! PINK's empty and error states as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The Sidebar, Agent Inbox and Decision Inbox draw their empty
//! lists with its note; this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/empty-state/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, SharedString, Window, div, prelude::*, px, svg};

use crate::{
    HoverFill,
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "Empty State";
pub const DESCRIPTION: &str = "PINK's empty and error states as a package the app pins: a surface with nothing to show centres a glyph tile, a title, a description and one bordered action that becomes Try again after a failure, and an empty list says so in a centred title with the next step under it";

/// Pink: `icons::GLOBE`, the Browser place's glyph.
const ICON_GLOBE: &str = "icons/empty-state/globe.svg";

/// One pane state as PINK's Browser surface draws it (browser view.rs
/// `empty_body`): before a page, or after one failed to load.
fn browser_pane(key: &'static str, error: Option<&'static str>, t: &Pink) -> impl IntoElement {
    let (heading, copy, label): (&str, SharedString, &str) = match error {
        Some(error) => ("Couldn’t load this page", error.into(), "Try again"),
        None => (
            "Preview your work",
            "Preview your local app or keep a website beside your conversation.".into(),
            "Enter an address",
        ),
    };
    let action_key = format!("{key}-action");
    pane().child(
        column()
            .child(
                tile(&TileStyle {
                    edge: t.border,
                    fill: t.surface_raised.opacity(0.5),
                })
                .child(
                    svg()
                        .path(ICON_GLOBE)
                        .size(ui_rems(TILE_GLYPH))
                        .text_color(t.text_muted),
                ),
            )
            .child(title(t.text).child(heading))
            .child(description(t.text_muted).child(copy))
            .child(
                action(&ActionStyle {
                    edge: t.border,
                    text: t.text,
                })
                .id(SharedString::from(action_key.clone()))
                .cursor_pointer()
                .role(Role::Button)
                .aria_label(if error.is_some() {
                    "Retry page"
                } else {
                    "Enter an address"
                })
                .hover_fill(
                    action_key,
                    t.surface_raised,
                    pink::flatten(t.fill(FillTier::Tertiary), t.surface_raised),
                )
                .child(label)
                .when(error.is_none(), |el| {
                    el.child(shortcut(t.text_faint).child("⌘L"))
                }),
            ),
    )
}

/// A list note as PINK's sidebar draws it (shell.rs "sidebar-empty-state"),
/// announced as a status.
fn list_note(
    key: &'static str,
    heading: &'static str,
    hint: &'static str,
    t: &Pink,
) -> impl IntoElement {
    note(
        &NoteStyle {
            title: t.readable_muted(0.9),
            hint: t.readable_faint(0.9),
        },
        heading,
        hint,
    )
    .id(key)
    .role(Role::Status)
    .aria_label(format!("{heading}. {hint}"))
}

/// A bordered stand-in for the surface a state fills.
fn surface(width: f32, height: f32, t: &Pink) -> gpui::Div {
    div()
        .w(px(width))
        .h(px(height))
        .flex()
        .flex_col()
        .rounded(px(pink::radius::MD))
        .border_1()
        .border_color(t.border)
        .bg(t.canvas())
        .overflow_hidden()
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
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(16.0))
                    .child(surface(360.0, 300.0, &t).child(browser_pane(
                        "story-empty-browser",
                        None,
                        &t,
                    )))
                    .child(surface(360.0, 300.0, &t).child(browser_pane(
                        "story-empty-browser-error",
                        Some("The server at localhost:3000 isn’t responding."),
                        &t,
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(16.0))
                    .child(surface(260.0, 120.0, &t).child(list_note(
                        "story-empty-threads",
                        "No threads yet",
                        "Start one with ⌘N.",
                        &t,
                    )))
                    .child(surface(260.0, 120.0, &t).child(list_note(
                        "story-empty-attention",
                        "Nothing needs attention",
                        "Running and waiting threads appear here.",
                        &t,
                    ))),
            )
    })
}
