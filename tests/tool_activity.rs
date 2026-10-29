//! The `pink-tool-activity` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its tool activity out (UI rethink D63). Failure modes: a
//! header off the 28 pt pitch, or one that stays fixed while its 14/20 text
//! grows; a long command that pushes the failure count, the state word or
//! the chevron out of the row instead of truncating; a verb that truncates
//! before its object; inline code taller than the sentence's line box, so
//! the row's baseline jumps; a failure message that crowds its recovery
//! actions off the line instead of truncating before them.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, SharedString, TestAppContext, Window, div,
    prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::tool_activity::{
        ActivityState, ActivitySummary, FailureStyle, PITCH, RecoveryStyle, Segment,
        activity_header, failure_line, header_style, recovery_action, sentence, sentence_style,
    },
    pink::Pink,
};

/// The test column's width: narrow enough that the long command truncates.
const COLUMN: f32 = 320.0;

const LONG: &str =
    "cargo test --workspace --locked --all-features -- --nocapture transcript::activity";

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let s = self.ui / 16.0;
        let glyph = |name: &'static str| {
            div()
                .flex_none()
                .size(px(14.0 * s))
                .debug_selector(move || name.into())
                .into_any_element()
        };
        let verb = div()
            .flex_none()
            .debug_selector(|| "verb".into())
            .child("Running");
        let code = div()
            .min_w_0()
            .debug_selector(|| "code".into())
            .child(sentence(
                &[Segment::Code(SharedString::from(LONG))],
                &sentence_style(&t, s),
            ));
        let title = div()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(4.0 * s))
            .child(verb)
            .child(code)
            .into_any_element();
        let header = activity_header(
            "header",
            title,
            glyph("glyph"),
            glyph("chevron"),
            ActivitySummary {
                state: ActivityState::NeedsReview,
                failures: 2,
            },
            header_style(&t, s),
        )
        .debug_selector(|| "header".into());
        let recovery = recovery_action(
            "recover",
            "Show error".into(),
            &RecoveryStyle {
                scale: s,
                height: 20.0 * s,
                radius: 6.0,
                text: t.text,
            },
        )
        .debug_selector(|| "recover".into())
        .into_any_element();
        let failure = failure_line(
            SharedString::from(format!("error: {LONG} exited with status 101")),
            vec![recovery],
            &FailureStyle {
                scale: s,
                height: 24.0 * s,
                text_size: 13.0 * s,
                line_height: 18.0 * s,
                message: t.content_muted,
            },
        )
        .debug_selector(|| "failure".into());
        div()
            .w(px(COLUMN))
            .flex()
            .flex_col()
            .debug_selector(|| "column".into())
            .child(header)
            .child(failure)
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn activity_keeps_its_pitch_verb_and_trailing_facts_at_every_interface_size(
    cx: &mut TestAppContext,
) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let s = ui / 16.0;
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let column = bounds(cx, "column");
        let header = bounds(cx, "header");
        let chevron = bounds(cx, "chevron");
        let verb = bounds(cx, "verb");
        let code = bounds(cx, "code");
        let failure = bounds(cx, "failure");
        let recover = bounds(cx, "recover");
        let h = f32::from(header.size.height);
        assert!(
            near(h, PITCH * s),
            "ui {ui}: header {h} off the 28 pt pitch"
        );
        // The long command truncates: the chevron stays inside the row.
        assert!(
            f32::from(chevron.right()) <= f32::from(column.right()) + 0.5,
            "ui {ui}: chevron pushed out of the row"
        );
        assert!(
            f32::from(code.right()) <= f32::from(chevron.left()),
            "ui {ui}: inline code overlaps the trailing facts"
        );
        // The verb keeps its natural width (it never truncates first).
        assert!(
            f32::from(verb.size.width) > 30.0 * s,
            "ui {ui}: verb squeezed to {}",
            f32::from(verb.size.width)
        );
        // Inline code fits the 20 pt line box, so the baseline holds.
        assert!(
            f32::from(code.size.height) <= 20.0 * s + 0.5,
            "ui {ui}: inline code {} taller than the line",
            f32::from(code.size.height)
        );
        assert!(near(f32::from(failure.size.height), 24.0 * s), "ui {ui}");
        // The recovery action stays whole on the failure line.
        assert!(
            f32::from(recover.right()) <= f32::from(column.right()) + 0.5,
            "ui {ui}: recovery action pushed out of the line"
        );
        assert!(
            f32::from(recover.size.width) > 50.0 * s,
            "ui {ui}: recovery action squeezed"
        );
    }
}

#[test]
fn attention_states_alone_carry_a_word_and_names_follow_the_drawn_order() {
    for state in [
        ActivityState::Running,
        ActivityState::Thinking,
        ActivityState::Recorded,
    ] {
        assert_eq!(state.label(), None, "{state:?} repeats the sentence's verb");
    }
    let summary = ActivitySummary {
        state: ActivityState::Stopped,
        failures: 1,
    };
    assert_eq!(
        summary.description("Ran 2 commands"),
        "Ran 2 commands, 1 failed, Stopped"
    );
    let settled = ActivitySummary {
        state: ActivityState::Recorded,
        failures: 0,
    };
    assert_eq!(settled.description("Ran pwd"), "Ran pwd");
}
