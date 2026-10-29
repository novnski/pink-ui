//! File Badge: the `pink-file-badge` package (`components/file-badge`),
//! PINK's tool-header file badge as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The Tool Call Card story's file badges draw with it; this
//! module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/file-badge/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, SharedString, Window, div, img, prelude::*, px};

use crate::{
    motion,
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "File Badge";
pub const DESCRIPTION: &str = "PINK's tool-header file badge as a package the app pins: a 22px chip holding the file-type artwork in a counter-shaded 20px well and the file's name, which truncates; as a control it deepens under the pointer and opens the file, and a path it may not open stays a quiet, explained label";

const FILE_RUST: &str = "icons/tool-call-card/file-rust.svg";
const FILE_RUST_DARK: &str = "icons/tool-call-card/file-rust-dark.svg";
const FILE_DOCUMENT: &str = "icons/tool-call-card/file-document.svg";
const FILE_DOCUMENT_DARK: &str = "icons/tool-call-card/file-document-dark.svg";

/// One badge as PINK's tool header draws it (file_badges.rs
/// `render_file_badge`); `opens` makes it the open control.
fn file(
    key: &'static str,
    name_text: &'static str,
    rust: bool,
    opens: bool,
    t: &Pink,
) -> impl IntoElement {
    let artwork = match (rust, t.dark) {
        (true, true) => FILE_RUST_DARK,
        (true, false) => FILE_RUST,
        (false, true) => FILE_DOCUMENT_DARK,
        (false, false) => FILE_DOCUMENT,
    };
    let seat = pink::grey(if t.dark { 0 } else { 255 }).opacity(0.16);
    let hover_key = SharedString::from(format!("{key}-fade"));
    badge(&BadgeStyle {
        background: if opens {
            motion::hover_blend(&hover_key, t.ink(0.06), t.ink(0.11))
        } else {
            t.ink(0.06)
        },
        text: t.text.opacity(0.85),
    })
    .id(key)
    .when(opens, |el| {
        el.role(Role::Button)
            .aria_label(format!("Open {name_text} in Files"))
            .cursor_pointer()
            .on_hover(motion::hover_listener(hover_key))
    })
    .when(!opens, |el| {
        el.role(Role::Label)
            .aria_label(format!("{name_text}: outside this chat's folder"))
    })
    .child(well(seat, img(artwork).flex_none().size(px(GLYPH))))
    .child(name(name_text))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(12.0))
            .child(file(
                "story-file-badge-open",
                "file_badges.rs",
                true,
                true,
                &t,
            ))
            .child(file(
                "story-file-badge-label",
                "release-notes.md",
                false,
                false,
                &t,
            ))
            .child(div().w(px(120.0)).flex().child(file(
                "story-file-badge-narrow",
                "an-unusually-long-file-name.rs",
                true,
                true,
                &t,
            )))
    })
}
