//! Question Card: the `pink-question-card` package
//! (`components/question-card`), PINK's question card as a single-GPUI
//! package the app pins (UI rethink decision 54). The gallery compiles the
//! package's own source here, unchanged, against its GPUI; PINK compiles the
//! same file against its patched one. The Approval Card and Permission
//! Request stories draw with it through [`palette`] and [`option_style`];
//! this module adds those and the story.

#[allow(dead_code)]
mod package {
    include!("../../components/question-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Div, Hsla, IntoElement, SharedString, Window, div, prelude::*, px};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink, radius},
    story,
};

pub const NAME: &str = "Question Card";
pub const DESCRIPTION: &str = "PINK's question card as a package the app pins: the panel an agent's question or approval takes in the composer's place, with a tracked header and page counter, the question, its context as quiet labels over values, numbered options washed when picked, a nested answer field and a footer whose notes keep the primary action on the trailing edge";

/// The card's colours on Pink's theme (PINK composer.rs `render_wizard`).
pub fn palette(t: &Pink) -> Palette {
    Palette {
        text: t.text,
        header: t.readable_muted(0.6),
        counter_fill: t.ink(0.06),
        detail: t.readable_muted(0.65),
        note: t.readable_muted(0.7),
    }
}

/// One option row's colours: a deeper wash and a faint ink edge when
/// picked, otherwise `rest` (the host's hover-blended wash).
pub fn option_style(t: &Pink, picked: bool, rest: Hsla) -> OptionStyle {
    OptionStyle {
        border: if picked {
            t.ink(0.16)
        } else {
            gpui::transparent_black()
        },
        fill: if picked { t.ink(0.09) } else { rest },
        label: if picked { t.text } else { t.text.opacity(0.9) },
        detail: t.readable_muted(0.65),
        number_fill: if picked { t.ink(0.16) } else { t.ink(0.05) },
        number_text: if picked {
            t.text
        } else {
            t.readable_muted(0.6)
        },
    }
}

/// The resting option wash, blended over its hover fade.
pub fn option_rest(t: &Pink, hover_key: &str) -> Hsla {
    motion::hover_blend(hover_key, t.ink(0.025), t.ink(0.06))
}

/// The pill the card sits on (PINK composer.rs `composer_surface`, on the
/// frosted composer plate).
pub fn surface(t: &Pink) -> Div {
    div()
        .rounded(px(radius::COMPOSER_PILL))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
        .shadow(pink::shadow_lg())
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let p = palette(&t);
        let choice = |ix: usize, label: &'static str, detail: &'static str, picked: bool| {
            let key = format!("question-card-option-{ix}");
            option(
                &option_style(&t, picked, option_rest(&t, &key)),
                label,
                [SharedString::from(detail)],
                Some(ix + 1),
            )
            .id(SharedString::from(key.clone()))
            .on_hover(motion::hover_listener(key))
        };
        div()
            .w(px(560.))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(
                surface(&t)
                    .flex()
                    .flex_col()
                    .child(
                        body()
                            .child(header(
                                &p,
                                pink::tracked_upper("Release scope"),
                                Some("1 of 2".into()),
                            ))
                            .child(question(
                                "question-card-question",
                                &p,
                                "Which steps should ship in v0.5?",
                            ))
                            .child(context(
                                "question-card-context",
                                &p,
                                [(
                                    SharedString::from("Working directory"),
                                    SharedString::from("~/projects/release"),
                                )],
                            ))
                            .child(hint(&p, "Select one or more options."))
                            .child(
                                options("question-card-options")
                                    .child(choice(
                                        0,
                                        "Cache migration",
                                        "Additive keys, safe to land first.",
                                        true,
                                    ))
                                    .child(choice(
                                        1,
                                        "UI flag",
                                        "Behind the beta toggle until 0.6.",
                                        false,
                                    )),
                            )
                            .child(answer(
                                surface(&t),
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(14.))
                                    .line_height(px(22.75))
                                    .text_color(t.placeholder)
                                    .child("Type your own answer, or leave this blank to use the selected option"),
                            )),
                    )
                    .child(
                        footer()
                            .child(notes().child(note(
                                &p,
                                "PINK picks Cache migration in 40s unless you answer",
                            )))
                            .child(
                                pink::button(
                                    "question-card-next",
                                    ButtonKind::Primary,
                                    ControlSize::Lg,
                                    &t,
                                )
                                .px(px(ACTION_PAD_X))
                                .child("Next"),
                            ),
                    ),
            )
            .into_any_element()
    })
}
