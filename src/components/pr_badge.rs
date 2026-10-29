//! PR Badge: the `pink-pr-badge` package (`components/pr-badge`), PINK's
//! pull request badges as a single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The Git Actions card's PR badge and the Chat Composer's PR
//! chips draw through it. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/pr-badge/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, IntoElement, SharedString, Window, div, prelude::*, px};

use crate::{
    motion,
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "PR Badge";
pub const DESCRIPTION: &str = "PINK's pull request badges as a package the app pins: the compact sidebar badge with the number in mono, toned by the pull request's state, and the composer chip that leads with the pull request glyph and says Draft for an open draft, washed in its tone and brighter under the pointer";

const ICON_PULL_REQUEST: &str = "icons/git-actions/git-pull-request.svg";

/// A state's tone (Pink `ChangeRequestBadgeTone::color`).
#[derive(Clone, Copy)]
enum State {
    Open,
    Draft,
    Merged,
    Closed,
}

impl State {
    fn tone(self, t: &Pink) -> Hsla {
        match self {
            Self::Open => t.success,
            Self::Draft => t.text_muted,
            Self::Merged => t.code_text,
            Self::Closed => t.danger,
        }
    }
}

/// The host's resolved colours for a badge with a hover wash (Pink
/// `hover_fill_fg`): the tone at 8% easing to 16%, the label from the
/// readable tone to the tone itself.
pub fn hover_style(key: &str, tone: Hsla, t: &Pink) -> BadgeStyle {
    let p = motion::hover_t(key);
    BadgeStyle {
        fill: Some(motion::mix(tone.opacity(0.08), tone.opacity(0.16), p)),
        text: motion::mix(t.readable_tone(tone, 0.85), tone, p),
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let sidebar = |number: u32, state: State| {
            let key = format!("story-pr-sidebar-{number}");
            badge(Surface::Sidebar, &hover_style(&key, state.tone(&t), &t))
                .id(SharedString::from(key.clone()))
                .cursor_pointer()
                .on_hover(motion::hover_listener(key))
                .child(number_label(number))
        };
        let composer = |number: u32, state: State| {
            let key = format!("story-pr-composer-{number}");
            let tone = state.tone(&t);
            badge(Surface::Composer, &hover_style(&key, tone, &t))
                .id(SharedString::from(key.clone()))
                .cursor_pointer()
                .on_hover(motion::hover_listener(key))
                .child(glyph(ICON_PULL_REQUEST, t.readable_tone(tone, 0.85)))
                .child(number_label(number))
                .when(matches!(state, State::Draft), |chip| chip.child("Draft"))
        };
        // The task list's count badge: no plate, the label alone.
        let count = badge(
            Surface::Sidebar,
            &BadgeStyle {
                fill: None,
                text: t.readable_tone(t.success, 0.85),
            },
        )
        .child(number("Geist Mono", "PR 2"));
        let row = || div().flex().flex_row().items_center().gap(px(8.0));
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                row()
                    .child(sidebar(171, State::Open))
                    .child(sidebar(168, State::Merged))
                    .child(sidebar(152, State::Closed))
                    .child(count),
            )
            .child(
                row()
                    .child(composer(171, State::Open))
                    .child(composer(174, State::Draft))
                    .child(composer(168, State::Merged))
                    .child(composer(152, State::Closed)),
            )
            .into_any_element()
    })
}

fn number_label(n: u32) -> gpui::Div {
    number(pink::MONO, SharedString::from(format!("#{n}")))
}
