//! The context ring in the composer's controls row (Pink composer.rs
//! `usage_chip`, `usage_widths`, `controls_row_fit`; context_usage.rs
//! `chip_widths`) and where Compact and Clear live (Pink
//! `Pickers::context_ring_shown`, the model menu's `context_tray`; PINK
//! 0.31.3, D75), measured from the laid-out composer at Interface sizes 12
//! and 20. Failure modes: the ring drawn outside the controls row or over
//! Send; the share kept while the access mode drops to its glyph (PINK's
//! order drops the share first); a row that never shows the share at all;
//! the ring shown with the setting off, for a provider that reports no
//! usage, or in a new thread; the model menu dropping Compact and Clear
//! when there is no ring, or keeping them beside one.

use std::time::Duration;

use gpui::{
    Bounds, Context, IntoElement, Modifiers, Pixels, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        chat_composer::{ChatComposer, Checkout},
        context_window::{CompactState, ContextControls, ContextWindow},
    },
};

struct Host {
    width: f32,
    in_thread: bool,
    setting: bool,
    reports: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().pt(px(560.)).pl(px(40.)).child(
            ChatComposer::new("cc")
                .width(self.width)
                .in_thread(self.in_thread)
                .checkout(Checkout::branch("main"))
                .context(
                    ContextWindow::new("cw", "lumen-4", 200_000)
                        .system(3_100)
                        .conversation(96_300, 46)
                        .window_map(false),
                )
                .context_ring(self.setting, self.reports)
                .context_controls(ContextControls::new(CompactState::Ready)),
        )
    }
}

fn settle(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn mount(cx: &mut TestAppContext, ui: f32, host: Host) -> &mut VisualTestContext {
    cx.update(pink_ui::init);
    cx.update(|cx| {
        Theme::set_mode(ThemeMode::Dark, cx);
        Theme::set_ui_font_size(ui, cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| host);
    settle(cx);
    cx
}

fn inside(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x - px(0.5)
        && inner.right() <= outer.right() + px(0.5)
        && inner.origin.y >= outer.origin.y - px(0.5)
        && inner.bottom() <= outer.bottom() + px(0.5)
}

fn thread(width: f32) -> Host {
    Host {
        width,
        in_thread: true,
        setting: true,
        reports: true,
    }
}

#[gpui::test]
fn the_ring_rides_the_controls_row(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        let mut cx = cx.clone();
        let cx = mount(&mut cx, ui, thread(720.0));
        let controls = cx.debug_bounds("cc-controls").expect("the controls row");
        let ring = cx.debug_bounds("cw-chip").expect("the ring in a thread");
        let access = cx.debug_bounds("cc-access").expect("the access chip");
        let send = cx.debug_bounds("cc-send").expect("Send");
        assert!(
            inside(ring, controls),
            "the ring left the controls row at {ui}"
        );
        assert!(
            ring.origin.x >= access.right(),
            "the ring before access at {ui}"
        );
        assert!(ring.right() <= send.origin.x, "the ring over Send at {ui}");
        // A roomy row shows the share beside the ring.
        assert!(f32::from(ring.size.width) > 29.0, "no share on a wide row");
    }
}

#[gpui::test]
fn the_share_yields_before_the_access_words(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        let mut ring_alone = false;
        for width in (360..=720).step_by(8) {
            let mut cx = cx.clone();
            let cx = mount(&mut cx, ui, thread(width as f32));
            let ring = cx.debug_bounds("cw-chip").expect("the ring");
            let share = f32::from(ring.size.width) > 29.0;
            let words = cx.debug_bounds("cc-access-label").is_some();
            assert!(
                !share || words,
                "at {width} pt ({ui}) the share outlived the access words"
            );
            ring_alone |= !share && words;
        }
        assert!(
            ring_alone,
            "no width shows the ring alone beside the words at {ui}"
        );
    }
}

fn open_model_menu(cx: &mut VisualTestContext) {
    let chip = cx.debug_bounds("cc-model").expect("the model chip");
    cx.simulate_click(chip.center(), Modifiers::none());
    settle(cx);
}

#[gpui::test]
fn compact_and_clear_follow_the_ring(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        for (setting, reports) in [(true, true), (false, true), (true, false)] {
            let mut cx = cx.clone();
            let cx = mount(
                &mut cx,
                ui,
                Host {
                    width: 720.0,
                    in_thread: true,
                    setting,
                    reports,
                },
            );
            let ring = setting && reports;
            assert_eq!(cx.debug_bounds("cw-chip").is_some(), ring, "the ring");
            open_model_menu(cx);
            assert_eq!(
                cx.debug_bounds("cc-context").is_some(),
                !ring,
                "the model menu's Compact and Clear with setting {setting}, usage {reports}"
            );
            if !ring {
                for row in ["cc-context-compact", "cc-context-clear"] {
                    cx.debug_bounds(row).expect("a tray row");
                }
            }
        }
    }
}

#[gpui::test]
fn a_new_thread_has_no_ring(cx: &mut TestAppContext) {
    let cx = mount(
        cx,
        16.0,
        Host {
            width: 720.0,
            in_thread: false,
            setting: true,
            reports: true,
        },
    );
    assert!(cx.debug_bounds("cw-chip").is_none(), "a new thread's ring");
    open_model_menu(cx);
    assert!(
        cx.debug_bounds("cc-context").is_none(),
        "no chat to compact"
    );
}
