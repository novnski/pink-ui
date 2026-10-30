//! The `pink-autonomy-dial` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK's Permission mode menu and the gallery's Autonomy Dial group
//! an agent's actions with it. Failure modes: a heading or row that stops
//! scaling with the Interface size (fixed pixels clip its text at 20 or float
//! it at 12); a long action name that pushes the meta word (who decides) out
//! of a narrow menu instead of ending in an ellipsis; names that stop lining
//! up because a row without a glyph collapses its slot.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::autonomy_dial::{
        GROUP_GAP, HEAD_H, HeadingStyle, ROW_H, RowStyle, empty, group, groups, heading, row,
    },
    pink::Pink,
};

const LONG: &str = "Reach files outside the workspace and run commands that leave the sandbox";

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let head = HeadingStyle {
            title: t.text_muted,
            count: t.text_faint,
        };
        let style = RowStyle {
            icon: t.text_muted,
            label: t.text,
            meta: t.text_muted,
        };
        div().w(gpui::px(260.0)).child(
            groups()
                .child(
                    group()
                        .child(heading(&head, "Unattended", 2).debug_selector(|| "head-a".into()))
                        .child(
                            row(
                                &style,
                                Some("icons/autonomy-dial/file-text.svg".into()),
                                "Read files",
                                "Runs",
                            )
                            .child(div().size_0().debug_selector(|| "glyph-end".into()))
                            .debug_selector(|| "row-glyph".into()),
                        )
                        .child(
                            row(&style, None, LONG, "Asks you")
                                .child(div().size_0().debug_selector(|| "long-end".into()))
                                .debug_selector(|| "row-long".into()),
                        ),
                )
                .child(
                    group()
                        .child(
                            heading(&head, "Needs approval", 0).debug_selector(|| "head-b".into()),
                        )
                        .child(
                            empty(t.text_muted, "Nothing needs approval")
                                .debug_selector(|| "empty".into()),
                        ),
                ),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn groups_scale_with_the_interface_and_keep_who_decides_in_reach(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let head_a = bounds(cx, "head-a");
        assert!(
            (f32::from(head_a.size.height) - HEAD_H * scale).abs() < 0.5,
            "heading is {:?} tall at {ui}",
            head_a.size.height
        );
        for name in ["row-glyph", "row-long", "empty"] {
            let row = bounds(cx, name);
            assert!(
                (f32::from(row.size.height) - ROW_H * scale).abs() < 0.5,
                "{name} is {:?} tall at {ui}, expected {}",
                row.size.height,
                ROW_H * scale
            );
        }
        // A long name ends in an ellipsis: the row's last child stays inside
        // the 260 px menu column.
        let long = bounds(cx, "row-long");
        let end = bounds(cx, "long-end");
        assert!(
            end.origin.x <= long.origin.x + long.size.width,
            "who decides pushed out at {ui}: {:?} past {:?}",
            end.origin.x,
            long.origin.x + long.size.width
        );
        // The second group starts one gap below the first group's end.
        let head_b = bounds(cx, "head-b");
        let first_end = long.origin.y + long.size.height;
        assert!(
            (f32::from(head_b.origin.y - first_end) - GROUP_GAP * scale).abs() < 0.5,
            "group gap at {ui}: {:?}",
            head_b.origin.y - first_end
        );
    }
}
