//! Working Trailer: the `pink-working-trailer` package
//! (`components/working-trailer`), PINK's live line under the last
//! transcript row as a single-GPUI package the app pins (UI rethink decision
//! 54). The gallery compiles the package's own source here, unchanged,
//! against its GPUI; PINK compiles the same file against its patched one.
//! [`crate::pink::loaders`]' orb, status and timer and the Thinking Loader's
//! trailer draw through it; this module adds the story, one still trailer
//! per state PINK shows.

#[allow(dead_code)]
mod package {
    include!("../../components/working-trailer/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Div, SharedString, Window, div, prelude::*, px, transparent_black};

use crate::{
    pink::{
        self, Pink,
        loaders::{format_elapsed, tabular_figures},
    },
    story,
};

pub const NAME: &str = "Working Trailer";
pub const DESCRIPTION: &str = "PINK's working line as a package the app pins: the breathing orb on a one-line box, a status verb and its object, the elapsed timer in tabular figures and a quiet Stop that shows under the pointer";

/// One still trailer state.
struct Still {
    key: &'static str,
    verb: &'static str,
    object: Option<&'static str>,
    warning: bool,
    sentence: bool,
    secs: Option<i64>,
    stop: bool,
}

/// A still trailer: the orb at full breath, the status, the timer (unless a
/// held send has none) and the Stop (shown, as under the pointer).
fn still(s: Still, t: &Pink) -> Div {
    let tone = if s.warning {
        t.warning
    } else {
        t.content_muted
    };
    let line = status_line(SharedString::from(format!("{}-status", s.key)))
        .child(verb(s.verb, tone, s.sentence))
        .when_some(s.object, |el, text| el.child(object(text, t.text)));
    div().child(
        trailer(s.key)
            .child(orb_slot(t.accent, 1.0))
            .child(line)
            .when_some(s.secs, |el, secs| {
                el.child(
                    timer(format_elapsed(secs), t.content_faint).font_features(tabular_figures()),
                )
            })
            .when(s.stop, |el| {
                el.child(
                    stop(
                        SharedString::from(format!("{}-stop", s.key)),
                        "Stop",
                        t.content_muted,
                        transparent_black(),
                    )
                    .opacity(1.0),
                )
            }),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let states = [
            Still {
                key: "thinking",
                verb: "Thinking",
                object: None,
                warning: false,
                sentence: false,
                secs: Some(4),
                stop: true,
            },
            Still {
                key: "running",
                verb: "Running",
                object: Some("cargo test --workspace"),
                warning: false,
                sentence: false,
                secs: Some(72),
                stop: true,
            },
            Still {
                key: "sending",
                verb: "Sending",
                object: None,
                warning: false,
                sentence: false,
                secs: None,
                stop: false,
            },
            Still {
                key: "queued",
                verb: "Queued, will send automatically",
                object: None,
                warning: true,
                sentence: true,
                secs: None,
                stop: false,
            },
        ];
        div()
            .w(px(480.))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .children(states.map(|s| still(s, &t)))
    })
}
