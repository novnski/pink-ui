//! Markdown List: the `pink-markdown-list` package (`components/markdown-list`),
//! PINK's Markdown list markers as a single-GPUI package the app pins. The
//! gallery compiles the package's own source here, unchanged, against its
//! GPUI; PINK compiles the same file against its patched one. Markdown
//! Message's lists draw their discs, numbers and task checkboxes through it;
//! this module adds the theme's task colours and the story.

#[allow(dead_code)]
mod package {
    include!("../../components/markdown-list/src/lib.rs");
}
pub use package::*;

use gpui::{
    AnyElement, AnyView, App, Role, SharedString, Toggled, Window, div, prelude::*, px, svg,
};

use crate::{
    pink::{self, Pink},
    story,
};

const CHECK: &str = "icons/markdown-message/check.svg";

/// Body text the markers line up with (PINK `MD_TEXT_SIZE` /
/// `MD_LINE_HEIGHT`, the 15/24 reading surface).
const TEXT_SIZE: f32 = 15.;
const LINE_HEIGHT: f32 = 24.;

pub const NAME: &str = "Markdown List";
pub const DESCRIPTION: &str = "PINK's Markdown list markers as a package the app pins: accent discs centred on the first line, tinted numbers, and task items drawn as real checkboxes, toggleable in an editable document and at half strength in a read-only reply";

/// The theme's task colours (PINK `markdown/render.rs` task markers).
pub fn task_style(t: &Pink) -> TaskStyle {
    TaskStyle {
        edge: t.border,
        plate: t.accent,
        check: t.bg,
    }
}

/// A task's box with the semantics a host adds: the CheckBox role, its
/// toggled state and the item's text as its name.
fn task(t: &Pink, key: &str, checked: bool, toggleable: bool, label: &'static str) -> AnyElement {
    marker_slot(LINE_HEIGHT)
        .child(
            task_box(&task_style(t), checked, toggleable, svg().path(CHECK))
                .id(SharedString::from(key.to_owned()))
                .role(Role::CheckBox)
                .aria_toggled(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .aria_label(label),
        )
        .into_any_element()
}

fn text(label: &'static str) -> gpui::Div {
    body().child(
        div()
            .text_size(ui_rems(TEXT_SIZE))
            .line_height(ui_rems(LINE_HEIGHT))
            .child(label),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let numeral = t.readable_tone(t.accent, 1.);
        let column = |title: &'static str, toggleable: bool| {
            div()
                .w(px(300.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(pink::text::CAPTION))
                        .text_color(t.content_muted)
                        .child(title),
                )
                .child(
                    list()
                        .child(item(
                            task(
                                &t,
                                &format!("{title}-a"),
                                true,
                                toggleable,
                                "Ship the build",
                            ),
                            text("Ship the build"),
                        ))
                        .child(item(
                            task(
                                &t,
                                &format!("{title}-b"),
                                false,
                                toggleable,
                                "Write the notes",
                            ),
                            text("Write the notes"),
                        ))
                        .child(item(disc(t.accent, LINE_HEIGHT), text("A plain bullet")))
                        .child(item(
                            number("3.", numeral, TEXT_SIZE, LINE_HEIGHT),
                            text("A numbered step"),
                        )),
                )
        };
        div()
            .flex()
            .flex_row()
            .gap(px(32.))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(column("In a reply", false))
            .child(column("In a document", true))
    })
}
