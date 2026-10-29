//! Tool Activity: the `pink-tool-activity` package
//! (`components/tool-activity`), PINK's verb-first tool activity (UI rethink
//! D63) as a single-GPUI package the app pins. The gallery compiles the
//! package's own source here, unchanged, against its GPUI; PINK compiles the
//! same file against its patched one. This module adds the theme's tokens
//! and the story.

#[allow(dead_code)]
mod package {
    include!("../../components/tool-activity/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, SharedString, Window, div, prelude::*, px, svg};

use crate::{
    pink::{self, FillTier, Pink},
    story,
};

const TERMINAL: &str = "icons/tool-call-card/terminal.svg";
const FILE: &str = "icons/tool-call-card/file-text.svg";
const LAYERS: &str = "icons/tool-call-card/layers-minimalistic.svg";
const CHEVRON: &str = "icons/tool-call-card/alt-arrow-right.svg";

pub const NAME: &str = "Tool Activity";
pub const DESCRIPTION: &str = "PINK's tool activity as a package the app pins: one verb-first sentence per call or group on a 28 pt pitch at 14/20, commands and patterns as inline code, the verb kept while long objects truncate, a failure count and attention state only when the sentence cannot say them, and the failure line under a failed call with its recovery actions";

/// The 28 pt activity pitch at the 16 px default.
pub const PITCH: f32 = 28.0;

/// The theme's sentence tokens at `scale` (Interface size / 16).
pub fn sentence_style(t: &Pink, scale: f32) -> SentenceStyle {
    SentenceStyle {
        scale,
        text_size: 14.0 * scale,
        line_height: 20.0 * scale,
        verb: Some(t.content_muted),
        object: t.text,
        code: t.text,
        code_fill: t.fill(FillTier::Tertiary),
        code_font: pink::MONO.into(),
        code_size: 13.0 * scale,
    }
}

/// The theme's header tokens at `scale`.
pub fn header_style(t: &Pink, scale: f32) -> HeaderStyle {
    HeaderStyle {
        scale,
        height: PITCH * scale,
        radius: 6.0,
        left_inset: 6.0 * scale,
        text_size: 14.0 * scale,
        line_height: 20.0 * scale,
        foreground: t.content_muted,
        muted: t.content_faint,
        active: t.text,
        danger: t.danger,
        warning: t.warning,
    }
}

fn text(run: &'static str) -> Segment {
    Segment::Text(SharedString::from(run))
}

fn code(run: &'static str) -> Segment {
    Segment::Code(SharedString::from(run))
}

fn object(run: &'static str) -> Segment {
    Segment::Object(SharedString::from(run))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let icon = |path: &'static str| {
            svg()
                .path(path)
                .flex_none()
                .size(px(14.))
                .text_color(t.content_muted)
                .into_any_element()
        };
        let header = |id: &'static str,
                      glyph: &'static str,
                      segments: Vec<Segment>,
                      state: ActivityState,
                      failures: usize| {
            activity_header(
                id,
                sentence(&segments, &sentence_style(&t, 1.0)).into_any_element(),
                icon(glyph),
                icon(CHEVRON),
                ActivitySummary { state, failures },
                header_style(&t, 1.0),
            )
        };
        let failed = div()
            .flex()
            .flex_col()
            .child(div().h(px(PITCH)).flex().items_center().child(sentence(
                &[text("Failed to run"), code("cargo test --workspace")],
                &sentence_style(&t, 1.0),
            )))
            .child(failure_line(
                "error[E0425]: cannot find value `widht` in this scope".into(),
                vec![
                    recovery_action(
                        "show-error",
                        "Show error".into(),
                        &RecoveryStyle {
                            scale: 1.0,
                            height: 20.0,
                            radius: 6.0,
                            text: t.text,
                        },
                    )
                    .into_any_element(),
                    recovery_action(
                        "copy-command",
                        "Copy command".into(),
                        &RecoveryStyle {
                            scale: 1.0,
                            height: 20.0,
                            radius: 6.0,
                            text: t.text,
                        },
                    )
                    .into_any_element(),
                ],
                &FailureStyle {
                    scale: 1.0,
                    height: 24.0,
                    text_size: 13.0,
                    line_height: 18.0,
                    message: t.content_muted,
                },
            ));
        div()
            .w(px(420.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .font_family(pink::SANS)
            .child(header(
                "ran",
                TERMINAL,
                vec![text("Ran"), code("pwd")],
                ActivityState::Recorded,
                0,
            ))
            .child(header(
                "running",
                TERMINAL,
                vec![
                    text("Running"),
                    code("cargo test --workspace --locked -- --nocapture transcript"),
                ],
                ActivityState::Running,
                0,
            ))
            .child(header(
                "edited",
                FILE,
                vec![text("Edited"), object("2 files")],
                ActivityState::Recorded,
                1,
            ))
            .child(header(
                "searched",
                LAYERS,
                vec![
                    text("Searched for"),
                    code("TODO"),
                    text("in"),
                    object("crates/"),
                ],
                ActivityState::Stopped,
                0,
            ))
            .child(failed)
    })
}
