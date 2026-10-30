//! The `pink-agent-plan` package at Interface sizes 12, 16 and 20, as PINK's
//! composer and the gallery's Agent Plan stack a plan on the composer with
//! it. Failure modes: the header bar, a step row or the checklist's
//! disclosure row that stops scaling with the Interface size (fixed pixels
//! clip text at 20 or float it at 12); a long objective that pushes the step
//! count or Run plan out of the header, or a long step title that pushes the
//! row's Remove and Rename out of the tray instead of ending in an ellipsis;
//! a checklist line that stops wrapping, so a long step runs off the
//! composer; a run summary that names the wrong step or paints the wrong
//! word red; a reorder that drops or duplicates a step when the drop lands
//! past the end.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::agent_plan::{
        CHECKLIST_ROW_H, CLOSE, ChecklistStyle, HEADER_H, HeaderStyle, ROW_H, StepStatus,
        StepStyle, checklist, checklist_row, close, controls, disclosure, drop_index, header,
        header_summary, line, lines, move_step, step_text, step_title, summary, ui_rems,
    },
    pink::Pink,
};

const LONG: &str = "Point every service config in the staging cluster at the newly minted database secret and confirm each restart";

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let header_style = HeaderStyle {
            glyph: t.text_muted,
            label: t.text,
            objective: t.text_muted,
            facts: t.text_muted,
        };
        let step_style = StepStyle {
            title: t.text,
            detail: t.text_muted,
        };
        let checklist_style = ChecklistStyle {
            text: t.text_muted,
            danger: t.danger,
        };
        let run = summary([
            (StepStatus::Done, "Snapshot"),
            (StepStatus::Failed, LONG),
            (StepStatus::Pending, "Verify"),
        ]);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div().w(gpui::px(360.0)).child(
                    header()
                        .debug_selector(|| "header".to_owned())
                        .child(header_summary(
                            &header_style,
                            "icons/agent-plan/checklist.svg",
                            "Plan",
                            LONG,
                            ["12 steps".into(), "about 14m".into()],
                        ))
                        .child(
                            controls().child(
                                div()
                                    .w(ui_rems(72.0))
                                    .h(ui_rems(24.0))
                                    .debug_selector(|| "run".to_owned()),
                            ),
                        ),
                ),
            )
            .child(
                div()
                    .w(gpui::px(360.0))
                    .h(ui_rems(ROW_H))
                    .flex()
                    .items_center()
                    .debug_selector(|| "row".to_owned())
                    .child(step_text(
                        &step_style,
                        step_title(step_style.title, LONG),
                        Some("repo.edit, about 2s".into()),
                    ))
                    .child(
                        div()
                            .flex_none()
                            .w(ui_rems(59.0))
                            .h(ui_rems(28.0))
                            .debug_selector(|| "actions".to_owned()),
                    ),
            )
            .child(
                div().w(gpui::px(360.0)).child(
                    checklist(&checklist_style)
                        .child(
                            checklist_row()
                                .child(
                                    disclosure(
                                        &checklist_style,
                                        "icons/agent-plan/alt-arrow-right.svg",
                                        &run,
                                    )
                                    .debug_selector(|| "disclosure".to_owned()),
                                )
                                .child(
                                    close(&checklist_style, "icons/agent-plan/close.svg")
                                        .debug_selector(|| "close".to_owned()),
                                ),
                        )
                        .child(
                            lines().child(
                                line(&checklist_style, StepStatus::Failed, LONG)
                                    .debug_selector(|| "line".to_owned()),
                            ),
                        ),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn right(b: Bounds<Pixels>) -> f32 {
    f32::from(b.origin.x + b.size.width)
}

#[gpui::test]
fn the_plan_scales_with_the_interface_and_keeps_its_controls_in_reach(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let header = bounds(cx, "header");
        assert!(
            (f32::from(header.size.height) - HEADER_H * scale).abs() < 0.5,
            "header is {:?} tall at {ui}, expected {}",
            header.size.height,
            HEADER_H * scale
        );
        // A long objective truncates: Run plan stays inside the bar.
        let run = bounds(cx, "run");
        assert!(
            right(run) <= right(header) + 0.5,
            "Run plan pushed out at {ui}: {} past {}",
            right(run),
            right(header)
        );

        // A long step title truncates: the row's actions stay inside it.
        let row = bounds(cx, "row");
        let actions = bounds(cx, "actions");
        assert!(
            right(actions) <= right(row) + 0.5,
            "step actions pushed out at {ui}: {} past {}",
            right(actions),
            right(row)
        );
        assert!((f32::from(row.size.height) - ROW_H * scale).abs() < 0.5);

        let disclosure = bounds(cx, "disclosure");
        assert!(
            (f32::from(disclosure.size.height) - CHECKLIST_ROW_H * scale).abs() < 0.5,
            "disclosure is {:?} tall at {ui}",
            disclosure.size.height
        );
        let close = bounds(cx, "close");
        assert!((f32::from(close.size.width) - CLOSE * scale).abs() < 0.5);
        assert!(
            right(close) <= 360.0 + 0.5,
            "close tile pushed out at {ui}: {}",
            right(close)
        );
        // A long step line wraps inside the composer column.
        let line = bounds(cx, "line");
        assert!(right(line) <= 360.0 + 0.5);
        assert!(
            f32::from(line.size.height) > 11.5 * scale * 2.0,
            "a long step line did not wrap at {ui}: {:?}",
            line.size.height
        );
    }
}

#[test]
fn the_run_summary_names_the_step_it_is_on() {
    let running = summary([
        (StepStatus::Done, "Snapshot"),
        (StepStatus::Running, "Mint"),
        (StepStatus::Pending, "Restart"),
    ]);
    assert_eq!(running.label, "1 of 3, In progress: Mint");
    assert_eq!(running.red, None);

    // A failure outranks the pending steps after it, and only "Failed" is
    // red.
    let failed = summary([
        (StepStatus::Done, "Snapshot"),
        (StepStatus::Failed, "Restart"),
        (StepStatus::Pending, "Verify"),
    ]);
    assert_eq!(failed.label, "1 of 3, Failed: Restart");
    assert_eq!(&failed.label[failed.red.clone().unwrap()], "Failed");

    let next = summary([(StepStatus::Done, "a"), (StepStatus::Pending, "b")]);
    assert_eq!(next.label, "1 of 2, Next: b");
    let complete = summary([(StepStatus::Done, "a"), (StepStatus::Done, "b")]);
    assert_eq!(complete.label, "2 of 2, Plan complete");
    let skipped = summary([(StepStatus::Done, "a"), (StepStatus::Skipped, "b")]);
    assert_eq!(skipped.label, "1 of 2, Plan finished");
}

#[test]
fn a_drop_reorders_without_losing_a_step() {
    let mut steps = vec!["a", "b", "c", "d"];
    assert!(move_step(&mut steps, 0, 2));
    assert_eq!(steps, ["b", "c", "a", "d"]);
    // Past the end lands last, never panics or drops a step.
    assert!(move_step(&mut steps, 1, 9));
    assert_eq!(steps, ["b", "a", "d", "c"]);
    assert!(!move_step(&mut steps, 2, 2));
    assert!(!move_step(&mut steps, 7, 0));
    assert_eq!(steps.len(), 4);
    // A pointer above or below the list picks the first or last slot.
    assert_eq!(drop_index(-4.0, ROW_H, 4), 0);
    assert_eq!(drop_index(ROW_H * 1.5, ROW_H, 4), 1);
    assert_eq!(drop_index(ROW_H * 40.0, ROW_H, 4), 3);
    assert_eq!(drop_index(10.0, ROW_H, 0), 0);
}
