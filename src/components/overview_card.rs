//! Overview Card: the `pink-overview-card` package
//! (`components/overview-card`), PINK's thread overview card as one
//! single-GPUI package the app pins (UI rethink decision 54). The gallery
//! compiles the package's own source here, unchanged, against its GPUI;
//! PINK compiles the same file against its patched one. PINK's thread
//! overview (`crates/ui/src/shell/thread_details.rs`) draws its card,
//! sections and rows through it and lays its conversation out with the
//! package's canvas geometry; this module adds the story.
//!
//! The story shows the card inline, as it sits beside a conversation wide
//! enough for a readable lane: the workspace, the project's scripts,
//! version control, agents and lineage, related chats and artifacts.

#[allow(dead_code)]
mod package {
    include!("../../components/overview-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, prelude::*, px};

use crate::{
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Overview Card";
pub const DESCRIPTION: &str = "PINK's thread overview as a package the app pins: a 280 px card that sits beside the conversation while a readable chat lane still fits (the chat stays centred, shifts left only as far as it must, and narrows only at the canvas's padding), and turns into a popover when it does not. Its sections hold the workspace and the editors that open it, the project's scripts, the branch and its changes and pull requests, agents and lineage, related chats and artifacts, at full, compact or essential density for the height it has";

const BRANCH: &str = "icons/right-panel/git-branch.svg";
const PULL_REQUEST: &str = "icons/git-actions/git-pull-request.svg";
const BOT: &str = "icons/agent-tree/bot.svg";
const SPLIT: &str = "icons/subagent-cards/columns-2.svg";
const DOCUMENT: &str = "icons/file-tree/file-document.svg";
const WIDGET: &str = "icons/subagent-cards/widget.svg";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let muted = t.text_muted;
        let rest = RowStyle {
            text: t.text,
            fill: gpui::transparent_black(),
        };
        let hovered = RowStyle {
            text: t.text,
            fill: t.element_hover,
        };
        let button = |label: &'static str| {
            div()
                .px(ui_rems(8.0))
                .py(ui_rems(3.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(t.border)
                .bg(t.surface_raised)
                .text_size(ui_rems(12.0))
                .child(label)
        };
        let workspace = section(None)
            .child(
                row(&rest)
                    .child(glyph(WIDGET, muted))
                    .child(label_with_detail(
                        "This machine",
                        Some("~/code/pink".into()),
                        muted,
                    )),
            )
            .child(
                actions()
                    .child(detail(muted, "Open in"))
                    .child(button("Zed"))
                    .child(button("Cursor"))
                    .child(button("VS Code")),
            )
            .child(heading(muted, "Scripts", Some(2)))
            .child(
                row(&hovered)
                    .child(dot(t.success))
                    .child(label("dev server"))
                    .child(detail(muted, "Running")),
            )
            .child(
                row(&rest)
                    .child(dot(muted.opacity(0.5)))
                    .child(label("test"))
                    .child(detail(muted, "Run")),
            );
        let version_control = section(Some(t.border))
            .child(
                row(&rest)
                    .child(glyph(BRANCH, muted))
                    .child(label_with_detail(
                        "feature/overview-lane",
                        Some("Worktree pink-overview".into()),
                        muted,
                    )),
            )
            .child(
                row(&rest)
                    .child(glyph(DOCUMENT, muted))
                    .child(label("4 files changed"))
                    .child(detail(muted, "1 staged")),
            )
            .child(
                actions()
                    .child(button("Commit"))
                    .child(button("Push"))
                    .child(button("Changes")),
            )
            .child(
                row(&rest)
                    .child(glyph(PULL_REQUEST, muted))
                    .child(label("#545 Overview lane"))
                    .child(detail(muted, "Open")),
            )
            .child(show_more(muted, "Show 2 more"));
        let agents = section(Some(t.border))
            .child(heading(muted, "Agents", Some(2)))
            .child(
                row(&rest)
                    .child(dot(t.accent))
                    .child(label("Explore the canvas layout"))
                    .child(detail(muted, "Running")),
            )
            .child(
                row(&rest)
                    .child(dot(muted.opacity(0.5)))
                    .child(label("Review the modal backdrop"))
                    .child(detail(muted, "Done")),
            )
            .child(heading(muted, "Lineage", None))
            .child(row(&rest).child(glyph(BOT, muted)).child(label_with_detail(
                "Plan the overview",
                Some("Main chat".into()),
                muted,
            )));
        let related = section(Some(t.border))
            .child(heading(muted, "Related chats", Some(2)))
            .child(
                row(&rest)
                    .child(glyph(SPLIT, muted))
                    .child(label("Modal click-away"))
                    .child(detail(muted, "Side chat")),
            )
            .child(
                row(&rest)
                    .child(glyph(SPLIT, muted))
                    .child(label("Related drag protections"))
                    .child(detail(muted, "Alongside")),
            )
            .child(heading(muted, "Artifacts", Some(1)))
            .child(
                row(&rest)
                    .child(glyph(DOCUMENT, muted))
                    .child(label("overview-lane.png"))
                    .child(detail(muted, "Image")),
            );
        let loading = section(Some(t.border))
            .child(heading(muted, "Artifacts", None))
            .child(note(muted, "Reading…"));
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(24.0))
            .child(
                card(&CardStyle {
                    border: t.border,
                    background: t.surface,
                    text: t.text,
                    font_family: pink::SANS.into(),
                    shadows: None,
                })
                .w(px(CARD_WIDTH))
                .child(workspace)
                .child(version_control)
                .child(agents)
                .child(related),
            )
            .child(
                card(&CardStyle {
                    border: t.border,
                    background: t.surface,
                    text: t.text,
                    font_family: pink::SANS.into(),
                    shadows: None,
                })
                .w(px(CARD_WIDTH))
                .child(
                    section(None).child(
                        row(&rest)
                            .child(glyph(BRANCH, muted))
                            .child(label("main"))
                            .child(detail(muted, "Essential")),
                    ),
                )
                .child(loading),
            )
    })
}
