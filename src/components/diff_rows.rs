//! Diff Rows: the `pink-diff-rows` package (`components/diff-rows`), PINK's
//! diff row chrome as one single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. Diff Viewer's notices, `@@` bands, notes, gutters, markers
//! and split halves draw through it; this module adds the theme's diff
//! colours and the story.
//!
//! A hunk's band is 28 px on the hunk fill in mono at the meta size; a
//! notice is 24 px at 11 pt; a line keeps a 3 px bar column, fitted
//! line-number gutters, a 16 px marker column (18 on a split half) and the
//! code, unified on one row or split around a 1 px hairline, with an empty
//! half filled flat.

#[allow(dead_code)]
mod package {
    include!("../../components/diff-rows/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, Window, div, prelude::*, px};

use crate::{
    pink::{self, Pink, TEXT_CONTRAST_TARGET},
    story,
};

pub const NAME: &str = "Diff Rows";
pub const DESCRIPTION: &str = "PINK's diff row chrome as a package the app pins: the notice under a file header, the hunk band, the note a line can carry, and a line's bar, gutters and marker around the code, unified on one row or split around a hairline";

/// The diff's code size, gutter and band size, note size and line height at
/// the default code font size (PINK changes.rs `DIFF_TEXT_SIZE` and
/// `DIFF_LINE_HEIGHT`).
pub const TEXT_SIZE: f32 = 12.0;
pub const META_SIZE: f32 = 11.0;
pub const NOTE_SIZE: f32 = 10.5;
pub const LINE_HEIGHT: f32 = 21.0;

/// Hunk header text, floored on the hunk band (PINK `diff_hunk_text`).
pub fn hunk_text(t: &Pink) -> Hsla {
    t.readable_on(t.text_faint, t.diff_hunk_bg, TEXT_CONTRAST_TARGET)
}

/// The kit's tokens as the package's host style, at the default code size.
pub fn diff_style(t: &Pink) -> DiffStyle {
    DiffStyle {
        mono: pink::MONO.into(),
        text_size: TEXT_SIZE,
        meta_size: META_SIZE,
        note_size: NOTE_SIZE,
        line_height: LINE_HEIGHT,
        hunk_fill: t.diff_hunk_bg,
        hunk_text: hunk_text(t),
        faint: t.text_faint,
        filler: t.ink(0.03),
        divider: t.hairline(0.06),
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let style = diff_style(&t);
        let wash = |tone: Hsla| Hsla {
            a: if t.dark { 0.10 } else { 0.08 },
            ..tone
        };
        let faint = t.readable_faint(0.8);
        let code = |text: &'static str| {
            div()
                .flex_1()
                .min_w_0()
                .pl(px(12.))
                .font_family(pink::MONO)
                .text_size(px(TEXT_SIZE))
                .line_height(px(LINE_HEIGHT))
                .text_color(t.text)
                .whitespace_nowrap()
                .child(text)
        };
        let unified = |old: Option<u32>,
                       new: Option<u32>,
                       mark: &'static str,
                       tone: Option<Hsla>,
                       text: &'static str| {
            let row_wash = tone.map(wash);
            let mark_color = tone.unwrap_or(t.readable_faint(0.5));
            unified_row(row_wash, false, &style)
                .child(accent_bar(None))
                .child(gutter(old, faint, GUTTER_WIDTH, &style))
                .child(gutter(new, faint, GUTTER_WIDTH, &style))
                .child(marker(mark, mark_color, MARKER_WIDTH, &style))
                .child(code(text))
        };
        let half = |number: u32, mark: &'static str, tone: Hsla, text: &'static str| {
            split_half(Some(wash(tone)))
                .child(accent_bar(None))
                .child(gutter(Some(number), faint, GUTTER_WIDTH, &style))
                .child(marker(mark, tone, SPLIT_MARKER_WIDTH, &style))
                .child(code(text).pl(px(6.)))
                .into_any_element()
        };
        div()
            .w(px(560.))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .child(notice_row("New file", &style))
            .child(hunk_header_row("@@ -1,4 +1,5 @@ pub fn add", &style))
            .child(unified(
                Some(1),
                Some(1),
                "\u{b7}",
                None,
                "/// Adds two numbers.",
            ))
            .child(unified(
                Some(2),
                None,
                "\u{2212}",
                Some(t.diff_del),
                "    a + b",
            ))
            .child(unified(
                None,
                Some(2),
                "+",
                Some(t.diff_add),
                "    a.saturating_add(b)",
            ))
            .child(meta_line_row(
                "\\ No newline at end of file",
                unified_meta_pad(GUTTER_WIDTH, 12.),
                &style,
            ))
            .child(split_row(
                half(2, "\u{2212}", t.diff_del, "    a + b"),
                half(2, "+", t.diff_add, "    a.saturating_add(b)"),
                false,
                &style,
            ))
            .child(split_row(
                split_filler(&style).into_any_element(),
                half(3, "+", t.diff_add, "    // saturates"),
                false,
                &style,
            ))
    })
}
