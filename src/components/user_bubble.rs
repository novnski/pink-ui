//! User Bubble: the `pink-user-bubble` package (`components/user-bubble`),
//! PINK's sent-prompt plate as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. This module adds the gallery's plate (its theme's fill and
//! light-mode edge) for the transcript stories, and the story.

#[allow(dead_code)]
mod package {
    include!("../../components/user-bubble/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, BoxShadow, Div, Window, div, point, prelude::*, px};

use crate::{pink::Pink, story};

pub const NAME: &str = "User Bubble";
pub const DESCRIPTION: &str = "PINK's sent-prompt plate as a package the app pins: the rounded, padded bubble a user message sits on, right-aligned and capped at 80% of the transcript column, with a hairline edge in light mode";

/// The theme's plate for a prompt in a `column`-wide transcript: Pink's
/// `user_bubble_bg` with light mode's 1 px inset edge.
pub fn plate(column: f32, t: &Pink) -> Div {
    bubble(&BubbleStyle {
        fill: t.user_bubble_bg(),
        edge: t.user_bubble_edge().map(|edge| {
            vec![BoxShadow {
                color: edge,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: true,
            }]
        }),
        text: t.text,
        max_width: px(column * MAX_WIDTH_FRACTION),
    })
}

/// The story's transcript column.
const COLUMN: f32 = 560.;

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let row = |text: &'static str| {
            div()
                .w_full()
                .flex()
                .justify_end()
                .child(plate(COLUMN, &t).child(text))
        };
        div()
            .w(px(COLUMN))
            .flex()
            .flex_col()
            .gap(px(16.))
            .font_family(crate::pink::SANS)
            .child(row("Can you find out why the checkout test is flaky?"))
            .child(row(
                "The payment helper polls with a fixed two-second timeout. Make it wait for the mock's health check, raise the timeout to ten seconds, then run the suite twenty times in a row and tell me what you see.",
            ))
    })
}
