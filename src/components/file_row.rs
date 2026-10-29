//! File Row: the `pink-file-row` package (`components/file-row`), PINK's
//! Files pane rows as one single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. File Tree draws its entries, search results, folder status
//! rows and text search rows through it; this module adds the story.
//!
//! Rows are 27 px tall. A tree entry is an inset, rounded pill indented
//! 14 px a level with a 14 pt disclosure slot (a folder's chevron turns a
//! quarter while open), the file-type artwork and the 11.5 pt name; a name
//! search result is the same row at full width; a lazily listed folder
//! shows Loading…, Empty folder, an error with its Retry, or Load more…
//! one level deeper; a text search lists each file (name, folder, count)
//! over its matching lines.

#[allow(dead_code)]
mod package {
    include!("../../components/file-row/src/lib.rs");
}
pub use package::*;

use gpui::{
    AnyView, App, ElementId, HighlightStyle, StyledText, Window, div, img, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "File Row";
pub const DESCRIPTION: &str = "PINK's Files pane rows as a package the app pins: a tree entry as an inset pill indented a level at a time with a turning chevron, artwork and name, the full-width name search result, a folder's loading, empty, error and load-more rows, and a text search's file and matching-line rows";

const CHEVRON: &str = "icons/file-tree/alt-arrow-right.svg";
const CHEVRON_DOWN: &str = "icons/file-tree/alt-arrow-down.svg";
const FOLDER: &str = "icons/file-tree/folder.svg";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let sans = gpui::SharedString::from(pink::SANS);
        let hover = t.fill(FillTier::Tertiary);
        let entry = |key: &'static str,
                     label: &'static str,
                     depth: usize,
                     folder: Option<bool>,
                     selected: Option<bool>,
                     ignored: bool| {
            let id = format!("file-row-{key}");
            let base = div().id(ElementId::Name(id.clone().into()));
            let base = match selected {
                Some(focused) => base.bg(if focused {
                    t.accent.opacity(0.22)
                } else {
                    t.wash(0.08)
                }),
                None => base.hover_wash(id, hover),
            };
            tree_entry(base, depth, ignored)
                .child(disclosure(folder.map(|open| {
                    tree_chevron(svg().path(CHEVRON).flex_none(), open, t.text_faint)
                })))
                .child(artwork(img(FOLDER)))
                .child(name(
                    label,
                    if selected.is_some() {
                        t.text
                    } else {
                        t.text_muted
                    },
                    sans.clone(),
                ))
        };
        let preview = StyledText::new("    stripe.webhooks.constructEvent(body, sig)")
            .with_highlights([(
                4..10,
                HighlightStyle {
                    color: Some(t.code_text),
                    background_color: Some(t.code_wash),
                    ..Default::default()
                },
            )]);
        div()
            .w(px(300.))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .child(entry("src", "src", 0, Some(true), Some(true), false))
            .child(entry("routes", "routes", 1, Some(false), None, false))
            .child(entry("ignored", ".env.local", 1, None, None, true))
            .child(
                status_row(div(), 2, StatusKind::Note)
                    .child(status_label("Loading…", t.text_faint)),
            )
            .child(
                status_row(div(), 1, StatusKind::Error)
                    .child(error_message(
                        "Permission denied",
                        t.readable_tone(t.danger, 0.82),
                    ))
                    .child(retry_label("Retry", t.text)),
            )
            .child(
                status_row(div(), 1, StatusKind::More)
                    .child(status_label("Load more…", t.text_muted)),
            )
            .child(
                search_entry(div().bg(t.fill(FillTier::Secondary)), 1)
                    .child(disclosure(Some(search_chevron(
                        svg().path(CHEVRON_DOWN),
                        t.text_faint,
                    ))))
                    .child(artwork(img(FOLDER)))
                    .child(name("webhooks.ts", t.text, sans.clone())),
            )
            .child(
                text_row(div(), TextRowKind::File, sans.clone())
                    .child(search_chevron(svg().path(CHEVRON_DOWN), t.text_faint))
                    .child(artwork(img(FOLDER)))
                    .child(text_file_name("webhooks.ts", t.text))
                    .child(text_file_folder("src/routes", t.text_faint))
                    .child(match_count("3", t.text_faint)),
            )
            .child(
                text_row(div(), TextRowKind::Match, sans)
                    .child(line_number("12", t.text_faint))
                    .child(match_line(t.text_muted, pink::MONO.into()).child(preview)),
            )
    })
}
