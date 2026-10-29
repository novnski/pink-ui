//! Agent Row: the `pink-agent-row` package (`components/agent-row`), PINK's
//! Agents list row as a single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The Agent Tree story draws its rows with it; this module
//! adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/agent-row/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, Window, div, prelude::*, px};

use crate::{
    pink::{self, Pink, loaders},
    story,
};

pub const NAME: &str = "Agent Row";
pub const DESCRIPTION: &str = "PINK's Agents list row as a package the app pins: a status dot, or the working spinner while it runs, the title and a quiet status word, then what the agent is doing beside its model, tokens, tool uses and runtime in a trailing column";

/// The colours PINK's Agents list gives a row (`shell/agents.rs`): the
/// title in the text colour (red when failed), the status word muted, the
/// activity muted and the figures fainter.
pub fn row_style(failed: bool, t: &Pink) -> RowStyle {
    RowStyle {
        title: if failed {
            t.readable_tone(t.danger, 1.0)
        } else {
            t.text
        },
        status: t.readable_muted(0.9),
        activity: t.readable_muted(0.85),
        figures: t.readable_faint(0.9),
    }
}

fn demo_row(
    key: &'static str,
    title: &'static str,
    status: &'static str,
    activity: &'static str,
    figures: Option<&'static str>,
    t: &Pink,
) -> impl IntoElement {
    let mark = match status {
        "Running" => {
            loaders::mini_glyph_spinner(format!("{key}-spin"), 2.0, t.glyph).into_any_element()
        }
        "Done" => dot(t.readable_tone(t.success, 1.0)).into_any_element(),
        "Failed" => dot(t.readable_tone(t.danger, 1.0)).into_any_element(),
        "Stopped" => dot(t.readable_tone(t.warning, 1.0)).into_any_element(),
        _ => dot(t.text_faint).into_any_element(),
    };
    agent_row(
        &row_style(status == "Failed", t),
        mark,
        RowText {
            title: title.into(),
            status: status.into(),
            activity: activity.into(),
            figures: figures.map(Into::into),
        },
    )
    .id(key)
    .role(Role::Group)
    .aria_label(format!("{title}, {status}"))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .w(px(pink::layout::SIDEBAR_MAX))
            .flex()
            .flex_col()
            .gap(px(LIST_GAP))
            .child(demo_row(
                "story-agent-row-running",
                "Query rewriter",
                "Running",
                "Rewrite the 118 raw SQL calls that rely on implicit casts",
                Some("lumen-4 large, 45.8k tokens, 31 tool uses, 56s"),
                &t,
            ))
            .child(demo_row(
                "story-agent-row-done",
                "Schema auditor",
                "Done",
                "Found 3 tables whose defaults differ from production",
                Some("lumen-4 mini, 12.4k tokens, 9 tool uses, 54s"),
                &t,
            ))
            .child(demo_row(
                "story-agent-row-failed",
                "Test runner",
                "Failed",
                "cargo test failed: 3 tests in db::pool time out",
                Some("lumen-4 mini, 3s"),
                &t,
            ))
            .child(demo_row(
                "story-agent-row-unobserved",
                "Docs writer",
                "Not observed",
                "Not observed",
                Some("Transcript unavailable, Usage not reported"),
                &t,
            ))
    })
}
