//! Code Block: the `pink-code-block` package (`components/code-block`),
//! PINK's fenced-code chrome as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. Markdown Message's fences draw their frame, header and
//! actions through it, and Tool Call Card's output segments draw their
//! medium-weight code surface header through it; this module adds the
//! theme's frame colours and the story.

#[allow(dead_code)]
mod package {
    include!("../../components/code-block/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, SharedString, Window, div, prelude::*, px, svg, transparent_black};

use crate::{
    pink::{self, Pink},
    story,
};

const COPY: &str = "icons/markdown-message/copy.svg";
const COPY_CHECK: &str = "icons/markdown-message/copy-check.svg";
const WRAP_TEXT: &str = "icons/markdown-message/wrap-text.svg";

pub const NAME: &str = "Code Block";
pub const DESCRIPTION: &str = "PINK's fenced-code chrome as a package the app pins: the rounded frame, the header strip with the language on the left, its square glyph action and Copy action that grows a Copied label, and the medium-weight strip over a tool's output or inline diff";

/// The theme's frame colours (PINK `code_block_frame`/`code_block_header`).
pub fn frame_style(t: &Pink) -> FrameStyle {
    FrameStyle {
        fill: t.ink(0.035),
        border: t.border,
        header_fill: t.ink(0.02),
        label: t.content_muted,
    }
}

const SAMPLE: &str = "fn main() {\n    println!(\"hello\");\n}";
const OUTPUT: &str = "running 12 tests\ntest result: ok. 12 passed; 0 failed";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let style = frame_style(&t);
        let copy = |key: &'static str, copied: bool| {
            copy_action(
                SharedString::from(format!("{key}-copy")),
                svg()
                    .path(if copied { COPY_CHECK } else { COPY })
                    .flex_none()
                    .size(px(COPY_ICON))
                    .text_color(t.content_muted)
                    .into_any_element(),
                copied.then(|| SharedString::from("Copied")),
                t.content_muted,
                transparent_black(),
            )
        };
        let code = |text: &'static str| {
            div()
                .px(px(12.))
                .py(px(10.))
                .font_family(pink::MONO)
                .text_size(px(12.5))
                .line_height(px(18.))
                .text_color(t.text)
                .child(text)
        };
        let block = |key: &'static str, language: Option<&'static str>, copied: bool| {
            let fit = icon_action(
                SharedString::from(format!("{key}-fit")),
                svg()
                    .path(WRAP_TEXT)
                    .size(px(ACTION_ICON))
                    .text_color(t.content_muted)
                    .into_any_element(),
                transparent_black(),
            );
            let actions = if language.is_some() {
                vec![fit.into_any_element(), copy(key, copied).into_any_element()]
            } else {
                Vec::new()
            };
            frame(
                key,
                header(language.map(SharedString::from), actions, style),
                style,
            )
            .child(code(SAMPLE))
        };
        // A tool's output segment: the medium "Output" caption and its Copy.
        let output = |key: &'static str, copied: bool| {
            let head = surface_header("Output", vec![copy(key, copied).into_any_element()], style)
                .font_family(pink::SANS);
            frame(key, Some(head), style).child(code(OUTPUT))
        };
        // An inline diff: the file name and its counts, no actions.
        let diff = surface_header(
            div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .child(div().min_w_0().truncate().child("src/lib.rs"))
                .child(div().flex_none().text_color(t.diff_add).child("+3"))
                .child(div().flex_none().text_color(t.diff_del).child("\u{2212}1")),
            Vec::new(),
            style,
        )
        .font_family(pink::SANS);
        div()
            .w(px(480.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .font_family(pink::SANS)
            .child(block("rust", Some("rust"), false))
            .child(block("copied", Some("rust"), true))
            .child(block("bare", None, false))
            .child(output("output", false))
            .child(output("output-copied", true))
            .child(frame("diff", Some(diff), style).child(code(SAMPLE)))
    })
}
