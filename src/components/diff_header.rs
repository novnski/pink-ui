//! Diff Header: the `pink-diff-header` package (`components/diff-header`),
//! PINK's Changes pane chrome around the diff rows as one single-GPUI
//! package the app pins (UI rethink decision 54). The gallery compiles the
//! package's own source here, unchanged, against its GPUI; PINK compiles the
//! same file against its patched one. Diff Viewer draws its file headers,
//! sticky header, summary strip, scope and ref triggers and its menus' rows
//! through it; this module adds the story.
//!
//! A file header is 38 px tall: the 12 pt disclosure turning a quarter as
//! the file opens, the 14 pt artwork, the mono path and the `+N -N` counts on
//! an `ink(0.025)` plate, a hairline over every header after the first; the
//! sticky copy sits on the opaque content plane with a `border` rule under
//! it. The summary strip says what is diffed and its totals. The pane header
//! carries the scope trigger, `{branch} -> {base}` (the longer name gives way
//! first), the Git chip and, for a pinned commit, its sha and subject.

#[allow(dead_code)]
mod package {
    include!("../../components/diff-header/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, img, prelude::*, px, svg};

use crate::{
    components::{
        icon_button::{IconButtonStyle, icon_button},
        surface_toolbar,
    },
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Diff Header";
pub const DESCRIPTION: &str = "PINK's Changes pane chrome as a package the app pins: file headers with a turning disclosure, artwork, mono path and counts, the sticky copy held over the list, the summary strip, and the pane header's scope trigger, branch to base selector, Git chip, pinned commit and menu rows";

const CHEVRON: &str = "icons/diff-viewer/alt-arrow-right.svg";
const DOWN: &str = "icons/diff-viewer/alt-arrow-down.svg";
const ARROW: &str = "icons/diff-viewer/arrow-right.svg";
const SPLIT: &str = "icons/diff-viewer/columns-2.svg";
const WRAP: &str = "icons/diff-viewer/wrap-text.svg";
const FOLD: &str = "icons/diff-viewer/fold-vertical.svg";
const RUST: &str = "icons/diff-viewer/file-rust.svg";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let mono = gpui::SharedString::from(pink::MONO);
        let header = |key: &str, placement: FileHeaderPlacement, file: &str, open: f32| {
            let sticky = placement == FileHeaderPlacement::Sticky;
            file_header(
                format!("diff-header-{key}"),
                placement,
                &FileHeaderStyle {
                    fill: if sticky {
                        pink::flatten(t.ink(0.025), t.bg)
                    } else {
                        t.ink(0.025)
                    },
                    rule: if sticky { t.border } else { t.hairline(0.04) },
                },
            )
            .child(disclosure(svg().path(CHEVRON), open, t.readable_muted(0.7)))
            .child(artwork(img(RUST)))
            .child(path(file.to_string(), mono.clone(), t.text_dim))
            .child(count("+12", mono.clone(), t.readable_tone(t.diff_add, 1.0)))
            .child(count(
                "\u{2212}4",
                mono.clone(),
                t.readable_tone(t.diff_del, 1.0),
            ))
        };
        let toggles = surface_toolbar_toggles(&t);
        div()
            .w(px(560.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(
                div().h(px(surface_toolbar::HEIGHT)).px(px(8.)).child(
                    header_row()
                        .child(scope_trigger(
                            "diff-header-scope",
                            "Branch changes",
                            svg().path(DOWN),
                            &ScopeTriggerStyle {
                                fill: t.wash(0.05),
                                text: t.text,
                                chevron: t.readable_muted(0.7),
                            },
                        ))
                        .child(ref_selector(
                            "feature/saturating-arithmetic",
                            svg().path(ARROW),
                            ref_trigger(
                                "diff-header-ref",
                                "main",
                                svg().path(DOWN),
                                &RefTriggerStyle {
                                    mono: mono.clone(),
                                    fill: t.wash(0.0),
                                    text: t.text,
                                    chevron: t.readable_muted(0.7),
                                },
                            ),
                            &RefSelectorStyle {
                                mono: mono.clone(),
                                branch: t.text_dim,
                                arrow: t.text_faint,
                            },
                        ))
                        .child(spacer())
                        .child(trailing().children(toggles)),
                ),
            )
            .child(
                div().h(px(surface_toolbar::HEIGHT)).px(px(8.)).child(
                    header_row()
                        .child(scope_trigger(
                            "diff-header-scope-2",
                            "Working tree",
                            svg().path(DOWN),
                            &ScopeTriggerStyle {
                                fill: t.wash(0.05),
                                text: t.text,
                                chevron: t.readable_muted(0.7),
                            },
                        ))
                        .child(spacer())
                        .child(trailing().child(git_chip(
                            "diff-header-git",
                            "Git",
                            t.wash(0.05),
                            t.text,
                        ))),
                ),
            )
            .child(
                div().h(px(surface_toolbar::HEIGHT)).px(px(8.)).child(
                    header_row()
                        .child(commit_sha(
                            "d5c3372",
                            mono.clone(),
                            t.ink(0.05),
                            t.text_muted,
                        ))
                        .child(commit_subject("Document the guide", t.text)),
                ),
            )
            .child(
                div().h(px(surface_toolbar::HEIGHT)).px(px(8.)).child(
                    header_row()
                        .child(history_title(
                            "diff-header-history",
                            "feature/saturate",
                            mono.clone(),
                            t.text_dim,
                        ))
                        .child(history_count(
                            div().text_color(t.text_muted).child("3 commits"),
                        ))
                        .child(history_trailing()),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        summary_strip(t.hairline(0.06))
                            .child(summary_label("3 Changed files vs main", t.text_muted))
                            .child(summary_count(
                                "+13",
                                mono.clone(),
                                t.readable_tone(t.diff_add, 1.0),
                            ))
                            .child(summary_count(
                                "\u{2212}5",
                                mono.clone(),
                                t.readable_tone(t.diff_del, 1.0),
                            ))
                            .child(spacer())
                            .child(partial_badge(
                                "Partial snapshot",
                                t.warning.opacity(0.08),
                                t.readable_tone(t.warning, 0.75),
                            )),
                    )
                    .child(header(
                        "sticky",
                        FileHeaderPlacement::Sticky,
                        "src/lib.rs",
                        1.0,
                    ))
                    .child(header(
                        "first",
                        FileHeaderPlacement::Row { first: true },
                        "src/main.rs",
                        1.0,
                    ))
                    .child(
                        header(
                            "binary",
                            FileHeaderPlacement::Row { first: false },
                            "assets/logo.png",
                            0.0,
                        )
                        .child(binary_tag("BIN", t.text_faint)),
                    ),
            )
            .child(
                pink::popover_card(&t)
                    .w(ui_rems(REF_MENU_WIDTH))
                    .child(
                        ref_list("diff-header-ref-list")
                            .child(
                                pink::menu_row("diff-header-ref-0", false, &t)
                                    .child(ref_row_label("feature/saturate", mono.clone())),
                            )
                            .child(
                                pink::menu_row("diff-header-ref-1", true, &t)
                                    .child(ref_row_label("main", mono.clone())),
                            ),
                    )
                    .child(ref_list_empty("No matching branches", t.text_faint)),
            )
            .child(
                pink::popover_card(&t).w(ui_rems(SCOPE_MENU_WIDTH)).child(
                    scope_list()
                        .child(
                            pink::menu_row("diff-header-scope-0", true, &t)
                                .child(scope_row_label("Working tree")),
                        )
                        .child(
                            pink::menu_row("diff-header-scope-1", false, &t)
                                .child(scope_row_label("Branch changes")),
                        ),
                ),
            )
    })
}

/// The pane header's icon toggles (PINK `header_toggle`): `pink-icon-button`
/// squares holding `wash(0.14)` while latched.
fn surface_toolbar_toggles(t: &Pink) -> Vec<gpui::AnyElement> {
    [
        ("split", SPLIT, true),
        ("wrap", WRAP, false),
        ("fold", FOLD, false),
    ]
    .into_iter()
    .map(|(key, glyph, on)| {
        icon_button(
            format!("diff-header-{key}"),
            surface_toolbar::control_glyph(
                svg().path(glyph),
                if on { t.text } else { t.readable_muted(0.7) },
            )
            .into_any_element(),
            &IconButtonStyle {
                size: surface_toolbar::CONTROL_SIZE,
                radius: surface_toolbar::CONTROL_RADIUS,
                wash: t.wash(if on { 0.14 } else { 0.0 }),
                enabled: true,
            },
        )
        .into_any_element()
    })
    .collect()
}
