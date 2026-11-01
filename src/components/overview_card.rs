//! Overview Card: the `pink-overview-card` package
//! (`components/overview-card`), PINK's thread overview card as one
//! single-GPUI package the app pins (UI rethink decision 54). The gallery
//! compiles the package's own source here, unchanged, against its GPUI;
//! PINK compiles the same file against its patched one. PINK's thread
//! overview (`crates/ui/src/shell/thread_details.rs`) draws its card,
//! sections and controls through it and lays its conversation out with the
//! package's canvas geometry; this module adds the story.
//!
//! The story shows the card inline, as it sits beside a conversation wide
//! enough for a readable lane, in T3 Code's thread details grammar: the
//! workspace (where it runs, Open in and Run as split controls), version
//! control (the branch select, the commit split, Changes with its line
//! counts, pull requests and Show more), lineage (the parent, a running
//! agent and the folded previous agents), related chats and artifacts. The
//! second card is the essential density a short canvas leaves.

#[allow(dead_code)]
mod package {
    include!("../../components/overview-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, SharedString, Window, div, prelude::*, px};

use crate::{
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Overview Card";
pub const DESCRIPTION: &str = "PINK's thread overview as a package the app pins, in T3 Code's thread details grammar: a 280 px card that sits beside the conversation while a readable chat lane still fits (the chat stays centred, shifts left only as far as it must, and narrows only at the canvas's padding), and turns into a popover when it does not. Every control is one 32 pt row with a 16 pt glyph and a 13 pt medium label; split controls pair an action with its options behind a chevron, select rows open a menu, and lists end in Show more. Sections hold the workspace and its editor and scripts, the branch, commit and changes with their line counts, pull requests, lineage with folded previous agents, related chats and artifacts, at full, compact or essential density for the height it has";

const ICONS: &str = "icons/overview-card";

fn icon(name: &str) -> SharedString {
    format!("{ICONS}/{name}.svg").into()
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let muted = t.text_muted;
        let label_text = t.text.opacity(0.8);
        let rest = RowStyle {
            text: label_text,
            fill: gpui::transparent_black(),
        };
        let hovered = RowStyle {
            text: label_text,
            fill: t.element_hover,
        };
        let quiet = RowStyle {
            text: muted.opacity(0.7),
            fill: gpui::transparent_black(),
        };
        let diff = DiffCountsStyle {
            addition: t.success,
            deletion: t.danger,
            muted,
            mono: pink::MONO.into(),
        };
        let chevron = || select_chevron(icon("alt-arrow-down"), muted);
        let split = |group: &'static str,
                     glyph_name: &str,
                     text: &'static str,
                     style: &RowStyle,
                     show_hairline: bool| {
            split_group(group)
                .child(
                    split_primary(style)
                        .child(glyph(icon(glyph_name), muted))
                        .child(label(text)),
                )
                .child(split_separator(group, t.border).when(show_hairline, |el| el.bg(t.border)))
                .child(split_secondary(style).child(glyph(icon("alt-arrow-down"), muted)))
        };
        let pull_request = |number: &'static str, tone: Hsla, state: &'static str| {
            row(&rest)
                .child(glyph(icon("git-pull-request"), tone))
                .child(label(number))
                .child(meta(muted, state))
        };
        let disabled = RowStyle {
            text: muted.opacity(0.7),
            fill: gpui::transparent_black(),
        };
        // Arranged as T3 Code's overview: no headings over the workspace and
        // version control; Lineage names itself only with a status.
        let workspace = section(None)
            .child(
                row(&rest)
                    .child(glyph(icon("laptop"), muted))
                    .child(label("Iulian's MacBook Pro 16"))
                    .child(glyph(icon("folder"), muted)),
            )
            .child(split(
                "story-open-in",
                "file-code",
                "Open in Zed",
                &rest,
                false,
            ))
            .child(split("story-run", "play", "Run dev server", &hovered, true))
            .child(
                row(&rest)
                    .child(glyph(icon("plus"), muted))
                    .child(label("Add project script")),
            );
        let version_control = section(Some(t.border))
            .child(
                select_row(&rest)
                    .child(
                        select_label()
                            .child(glyph(icon("git-branch"), muted))
                            .child(label("feature/overview-lane"))
                            .child(meta(muted, "pink-overview")),
                    )
                    .child(chevron()),
            )
            .child(pull_request("#545: Overview lane", t.success, "Open"))
            .child(pull_request("#541: Related drag", t.accent, "Merged"))
            .child(show_more(&quiet, icon("alt-arrow-down"), "Show 2 more"))
            .child(split("story-commit", "check", "Commit", &disabled, false))
            .child(
                row(&rest)
                    .child(glyph(icon("git-diff"), muted))
                    .child(label("Changes"))
                    .child(diff_counts(&diff, 128, 14)),
            );
        let lineage = section(Some(t.border))
            .child(heading(
                muted,
                "Lineage",
                Some(("1 running".into(), t.accent)),
            ))
            .child(
                row(&rest)
                    .child(glyph(icon("arrow-up"), muted))
                    .child(title("Plan the overview", t.text.opacity(0.85)))
                    .child(meta(muted, "Main chat")),
            )
            .child(
                row(&rest)
                    .child(glyph(icon("bot"), t.accent))
                    .child(title("Explore the canvas layout", t.text.opacity(0.85)))
                    .child(meta(muted, "Running")),
            )
            .child(
                row(&rest)
                    .child(glyph(icon("bot"), muted))
                    .child(label("Previous agents"))
                    .child(meta(muted, "3"))
                    .child(fold_chevron(icon("alt-arrow-down"), muted, false)),
            );
        let related = section(Some(t.border))
            .child(heading(muted, "Related chats", None))
            .child(
                row(&rest)
                    .child(glyph(icon("arrow-fork"), muted))
                    .child(title("Modal click-away", t.text.opacity(0.85)))
                    .child(meta(muted, "Side chat")),
            )
            .child(
                row(&rest)
                    .pl(ui_rems(ROW_PAD_X + INDENT))
                    .child(glyph(icon("bot"), muted))
                    .child(title("Backdrop blur probe", t.text.opacity(0.85)))
                    .child(meta(muted, "Side chat")),
            )
            .child(
                row(&rest)
                    .child(glyph(icon("columns-2"), muted))
                    .child(title("Related drag protections", t.text.opacity(0.85)))
                    .child(meta(muted, "Alongside")),
            );
        let artifacts = section(Some(t.border))
            .child(heading(muted, "Artifacts", None))
            .child(
                row(&rest)
                    .child(glyph(icon("file-image"), muted))
                    .child(label("overview-lane.png"))
                    .child(meta(muted, "Image")),
            )
            .child(
                row(&rest)
                    .child(glyph(icon("document"), muted))
                    .child(label("acceptance.pdf"))
                    .child(meta(muted, "PDF")),
            );
        let style = || CardStyle {
            border: t.border,
            background: t.surface_raised,
            text: t.text,
            font_family: pink::SANS.into(),
            shadows: None,
        };
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(24.0))
            .child(
                card(&style())
                    .w(px(CARD_WIDTH))
                    .child(workspace)
                    .child(version_control)
                    .child(lineage)
                    .child(related)
                    .child(artifacts),
            )
            .child(
                card(&style())
                    .w(px(CARD_WIDTH))
                    .child(
                        section(None)
                            .child(split("story-run-short", "play", "Run test", &rest, false))
                            .child(
                                select_row(&rest)
                                    .child(
                                        select_label()
                                            .child(glyph(icon("git-branch"), muted))
                                            .child(label("main")),
                                    )
                                    .child(chevron()),
                            )
                            .child(split("story-push", "cloud", "Push", &rest, false))
                            .child(
                                row(&rest)
                                    .child(glyph(icon("git-diff"), muted))
                                    .child(label("Changes"))
                                    .child(diff_counts(&diff, 0, 0)),
                            ),
                    )
                    .child(
                        section(Some(t.border))
                            .child(heading(muted, "Artifacts", None))
                            .child(note(muted, "Reading…")),
                    ),
            )
    })
}
