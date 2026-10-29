//! The `pink-diff-header` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays out the Changes pane. Failure modes: a file header that
//! grows with the Interface size (the virtualized list measures it at a
//! fixed 38 px, so rows would overlap) or loses its counts at the trailing
//! edge to a long path; a sticky copy without its rule; a ref selector where
//! the short base gives way before the long branch (it reads as a bare
//! ellipsis); controls that drift off the 28 px header height.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::diff_header::{
        ARTWORK, CONTROL_HEIGHT, FILE_HEADER_HEIGHT, FILE_HEADER_PAD_X, FileHeaderPlacement,
        FileHeaderStyle, RefSelectorStyle, RefTriggerStyle, STRIP_HEIGHT, ScopeTriggerStyle,
        artwork, count, disclosure, file_header, header_row, path, ref_selector, ref_trigger,
        scope_trigger, summary_label, summary_strip,
    },
};

const WIDTH: f32 = 420.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let black = gpui::black();
        let mono = gpui::SharedString::from("Menlo");
        let header = |name: &'static str, placement| {
            file_header(
                name,
                placement,
                &FileHeaderStyle {
                    fill: gpui::white(),
                    rule: black,
                },
            )
            .debug_selector(move || name.into())
            .child(disclosure(gpui::svg(), 1.0, black))
            .child(artwork(gpui::img("a.svg")).debug_selector(move || format!("{name}-art")))
            .child(path(
                "a/very/long/path/that/never/fits/inside/the/header/at/all/main.rs",
                mono.clone(),
                black,
            ))
            .child(
                count("+12", mono.clone(), black).debug_selector(move || format!("{name}-count")),
            )
        };
        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .child(
                summary_strip(black)
                    .debug_selector(|| "strip".into())
                    .child(summary_label("3 Uncommitted changes", black)),
            )
            .child(header("row", FileHeaderPlacement::Row { first: false }))
            .child(header("sticky", FileHeaderPlacement::Sticky))
            .child(
                div().w(px(300.0)).h(px(38.0)).child(
                    header_row()
                        .child(
                            scope_trigger(
                                "scope",
                                "Branch changes",
                                gpui::svg(),
                                &ScopeTriggerStyle {
                                    fill: black,
                                    text: black,
                                    chevron: black,
                                },
                            )
                            .debug_selector(|| "scope".into()),
                        )
                        .child(ref_selector(
                            "feature/a-very-long-branch-name-indeed",
                            gpui::svg(),
                            ref_trigger(
                                "base",
                                "main",
                                gpui::svg(),
                                &RefTriggerStyle {
                                    mono: mono.clone(),
                                    fill: black,
                                    text: black,
                                    chevron: black,
                                },
                            )
                            .debug_selector(|| "base".into()),
                            &RefSelectorStyle {
                                mono: mono.clone(),
                                branch: black,
                                arrow: black,
                            },
                        )),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Selectors are looked up by a 'static name.
    let key: &'static str = Box::leak(name.to_string().into_boxed_str());
    cx.debug_bounds(key)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn headers_keep_their_height_and_the_base_keeps_its_name(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let strip = bounds(cx, "strip");
        assert!(
            near(f32::from(strip.size.height), STRIP_HEIGHT),
            "strip at {ui}: {strip:?}"
        );
        for name in ["row", "sticky"] {
            let header = bounds(cx, name);
            let art = bounds(cx, &format!("{name}-art"));
            let count = bounds(cx, &format!("{name}-count"));
            // The rule is drawn inside the fixed height.
            assert!(
                near(f32::from(header.size.height), FILE_HEADER_HEIGHT),
                "{name} at {ui}: {header:?}"
            );
            assert!(
                near(f32::from(art.size.width), ARTWORK * scale),
                "{name} artwork at {ui}: {art:?}"
            );
            assert!(
                near(
                    f32::from(
                        header.origin.x + header.size.width - (count.origin.x + count.size.width)
                    ),
                    FILE_HEADER_PAD_X
                ),
                "{name} count at the trailing edge at {ui}: {count:?} in {header:?}"
            );
        }

        let scope = bounds(cx, "scope");
        let base = bounds(cx, "base");
        assert!(
            near(f32::from(scope.size.height), CONTROL_HEIGHT)
                && near(f32::from(base.size.height), CONTROL_HEIGHT),
            "controls at {ui}: {scope:?} {base:?}"
        );
        // "main" plus its chevron and pads: the long branch gave way instead.
        assert!(
            f32::from(base.size.width) > 30.0 * scale,
            "base kept its name at {ui}: {base:?}"
        );
    }
}
