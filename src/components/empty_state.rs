//! Empty State: the `pink-empty-state` package (`components/empty-state`),
//! PINK's empty and error states as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The Sidebar, Agent Inbox and Decision Inbox draw their empty
//! lists with its note; this module adds only the story: the Browser pane
//! state, a right-rail place with nothing open, the Related place's guide,
//! an empty model list and the sidebar's list notes.

#[allow(dead_code)]
mod package {
    include!("../../components/empty-state/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, SharedString, Window, div, prelude::*, px, svg};

use crate::{
    HoverFill,
    pink::{self, ButtonKind, ControlSize, FillTier, Pink},
    story,
};

pub const NAME: &str = "Empty State";
pub const DESCRIPTION: &str = "PINK's empty and error states as a package the app pins: a surface with nothing to show centres a glyph tile, a title, a description and one bordered action that becomes Try again after a failure, and an empty list says so in a centred title with the next step under it";

/// Pink: `icons::GLOBE`, the Browser place's glyph.
const ICON_GLOBE: &str = "icons/empty-state/globe.svg";
/// Pink: `icons::GIT_DIFF`, the Changes place's glyph.
const ICON_GIT_DIFF: &str = "icons/empty-state/git-diff.svg";
/// Pink: `icons::TERMINAL`, the Terminal place's glyph.
const ICON_TERMINAL: &str = "icons/empty-state/terminal.svg";
/// Pink: `icons::BOT` and `icons::FORK`, the related kinds' glyphs.
const ICON_BOT: &str = "icons/empty-state/bot.svg";
const ICON_FORK: &str = "icons/empty-state/arrow-fork.svg";
/// Pink: `icons::PLUS`.
const ICON_PLUS: &str = "icons/empty-state/plus.svg";

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

/// A right-rail place with nothing open, as PINK draws it (right_rail.rs
/// `render_place_empty`): the place's glyph, why it is empty, the notice
/// PINK adds when the place could not fill, and the button that opens one.
fn rail_place(
    key: &'static str,
    glyph: &'static str,
    message: &'static str,
    notice: Option<&'static str>,
    action: &'static str,
    t: &Pink,
) -> impl IntoElement {
    place()
        .id(key)
        .child(place_glyph(svg().path(glyph), t.readable_faint(0.8)))
        .child(place_message(t.text_muted).child(message))
        .when_some(notice, |el, notice| {
            el.child(place_notice(t.readable_muted(0.8)).child(notice))
        })
        .child(
            pink::button(
                format!("{key}-open"),
                ButtonKind::Secondary,
                ControlSize::Sm,
                t,
            )
            .aria_label(action)
            .child(pink::glyph(ICON_PLUS, ControlSize::Sm))
            .child(action),
        )
}

/// The Related place before any related chat exists, as PINK draws it
/// (related_chats.rs `render_related_empty`): what each kind is, then the
/// one the user can start.
fn related_guide(t: &Pink) -> impl IntoElement {
    let style = GuideStyle {
        title: t.text,
        text: t.text_muted,
    };
    guide(&style)
        .id("story-empty-related")
        .child(guide_title(&style).child("No related chats yet"))
        .child(guide_line(
            &style,
            svg().path(ICON_BOT),
            "Fresh side chats don't carry this chat's conversation. Start one with + (Side chat, Fresh), or this chat's agent may start one.",
        ))
        .child(guide_line(
            &style,
            svg().path(ICON_FORK),
            "Side chats with context carry this chat's context, captured when you start one; later messages here are not copied.",
        ))
        .child(
            guide_action().child(
                pink::button(
                    "story-empty-related-new",
                    ButtonKind::Ghost,
                    ControlSize::Sm,
                    t,
                )
                .aria_label("Side chat with context")
                .child(pink::glyph(ICON_PLUS, ControlSize::Sm))
                .child("Side chat with context"),
            ),
        )
}

/// A popover list with no rows, as PINK's model picker draws a search that
/// matched nothing (pickers.rs `empty_list_note`).
fn empty_menu(t: &Pink) -> impl IntoElement {
    let popup = t.for_popup();
    pink::popover_card(t).w(px(260.0)).flex().flex_col().child(
        popup_note(popup.text_muted, "No models found")
            .id("story-empty-models")
            .role(Role::Status)
            .aria_label("No models found"),
    )
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
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_start()
                    .gap(px(16.0))
                    .child(surface(300.0, 240.0, &t).child(rail_place(
                        "story-empty-changes",
                        ICON_GIT_DIFF,
                        "No diff open.",
                        None,
                        "Diff the working tree",
                        &t,
                    )))
                    .child(surface(300.0, 240.0, &t).child(rail_place(
                        "story-empty-terminal",
                        ICON_TERMINAL,
                        "No terminals open.",
                        Some("Saved terminals couldn’t be restored. Start a new one."),
                        "New terminal",
                        &t,
                    )))
                    .child(surface(300.0, 240.0, &t).child(related_guide(&t)))
                    .child(empty_menu(&t)),
            )
    })
}
