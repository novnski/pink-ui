//! Git Card: the `pink-git-card` package (`components/git-card`), PINK's Git
//! card chrome as one single-GPUI package the app pins (UI rethink decision
//! 54). The gallery compiles the package's own source here, unchanged,
//! against its GPUI; PINK compiles the same file against its patched one.
//! Git Actions draws its card, buttons, lines, file rows, checkpoints and
//! pull request panel through it; this module adds the story.
//!
//! The card opens full width over the diff on `card_glass_bg` with a
//! `border` rule under it, its lines 6 pt apart in 11 pt type. Buttons are
//! 26 pt outlined plates on `surface_raised`; the one that performs the
//! recommended step wears the accent plate with a medium label.

#[allow(dead_code)]
mod package {
    include!("../../components/git-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, FontWeight, Window, div, prelude::*, px};

use crate::{
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Git Card";
pub const DESCRIPTION: &str = "PINK's Git card chrome as a package the app pins: the card over the diff, branch and remote facts, the recommended step, pull request status and panel, file checkpoints, picked files with Stage and Unstage, and outlined buttons with the accent on the step to take";

/// A secondary card button's colours.
pub fn secondary(t: &Pink) -> ButtonStyle {
    ButtonStyle {
        fill: t.surface_raised,
        border: t.border,
        text: t.text,
        weight: None,
    }
}

/// The accent plate on the button that performs the recommended step.
pub fn primary(t: &Pink) -> ButtonStyle {
    ButtonStyle {
        fill: t.accent_strong,
        border: gpui::transparent_black(),
        text: t.on_accent,
        weight: Some(FontWeight::MEDIUM),
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let mono = gpui::SharedString::from(pink::MONO);
        let plate = CardStyle {
            border: t.border,
            fill: t.card_glass_bg(),
        };
        let file = |path: &'static str, picked: bool, staged: bool| {
            file_row()
                .child(pick_box(format!("git-card-pick-{path}"), picked, t.text))
                .child(file_path(path, mono.clone(), t.text))
                .child(if staged {
                    button(
                        format!("git-card-unstage-{path}"),
                        "Unstage",
                        &secondary(&t),
                    )
                } else {
                    button(format!("git-card-stage-{path}"), "Stage", &primary(&t))
                })
        };
        div()
            .w(px(520.))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(
                card("git-card-demo", &plate)
                    .child(
                        facts(t.text_dim)
                            .child("Branch: feature/saturate")
                            .child("Remote: origin")
                            .child(button("git-card-refresh", "Refresh", &secondary(&t))),
                    )
                    .child(next_step(
                        "git-card-next",
                        "Stage changes",
                        "2 changed files are not staged yet",
                        &NextStepStyle {
                            label: t.text,
                            reason: t.text_dim,
                        },
                    ))
                    .child(
                        status_row(t.text_dim)
                            .child(status_state("Open", t.readable_tone(t.success, 0.9)))
                            .child(status_title("Saturate the arithmetic helpers")),
                    )
                    .child(
                        panel("git-card-panel", &plate)
                            .child(panel_header(
                                "Pull request",
                                t.text,
                                button("git-card-close", "Close", &secondary(&t)),
                            ))
                            .child(pr_title(
                                "#482",
                                "Saturate the arithmetic helpers",
                                Some("Draft".into()),
                                &PrTitleStyle {
                                    quiet: t.text_dim,
                                    title: t.text,
                                },
                            ))
                            .child(pr_refs(
                                "feature/saturate \u{2192} main",
                                "Open",
                                t.text_dim,
                            ))
                            .child(pr_description(
                                "git-card-description",
                                "Adds saturating arithmetic.",
                                t.text,
                            ))
                            .child(check_row(
                                "git-card-check",
                                "Unit tests",
                                "success",
                                &CheckStyle {
                                    name: t.text,
                                    status: t.success,
                                },
                            ))
                            .child(field(30.0, t.border, div().px(px(6.)).child("main"))),
                    )
                    .child(
                        section(t.border)
                            .child(note("File checkpoints", t.text))
                            .child(
                                checkpoint_line(
                                    "Oct 20 12:00",
                                    "completed",
                                    &CheckpointStyle {
                                        stamp: t.text_dim,
                                        outcome: t.text_faint,
                                    },
                                )
                                .child(button(
                                    "git-card-diff",
                                    "Diff",
                                    &secondary(&t),
                                )),
                            )
                            .child(stack().child(note("2 changed paths", t.text_dim)).child(
                                patch(
                                    "git-card-patch",
                                    "@@ -1 +1 @@\n-a + b\n+a.saturating_add(b)",
                                    mono.clone(),
                                    t.text,
                                ),
                            )),
                    )
                    .child(
                        file_list("git-card-files")
                            .child(file("src/lib.rs", true, true))
                            .child(file("README.md", false, false)),
                    )
                    .child(message(
                        div().text_color(t.text_faint).child("Commit message"),
                    ))
                    .child(
                        button_row()
                            .child(button(
                                "git-card-generate",
                                "Generate message",
                                &secondary(&t),
                            ))
                            .child(button("git-card-commit", "Commit", &secondary(&t)))
                            .child(button("git-card-push", "Push", &secondary(&t))),
                    )
                    .child(note("Working\u{2026}", t.text_faint)),
            )
    })
}
