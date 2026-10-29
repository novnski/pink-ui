//! The context ring's Context menu (Pink pickers.rs `PickerKind::Context`,
//! `render_context_menu`, `render_context_controls`, `compact_context`,
//! `clear_context`; popover.rs `anchored_menu_above_leading`; PINK 0.31.1,
//! novnski/pink#195) and where Compact and Clear live without a ring (Pink
//! `Pickers::context_ring_shown`, PINK 0.31.3, D75), driven from the
//! laid-out elements at Interface sizes 12 and 20. Failure modes: the menu
//! centered over the ring instead of opening from its leading edge, or
//! covering it; the plain menu wider than PINK's 280 pt; arrows that stop at
//! the last row instead of wrapping; Enter activating a row the arrows didn't
//! reach; Clear acting on its first press; Escape leaving the confirmation
//! armed for the next open; a disabled or unresolved Compact still firing;
//! the ring shown for a provider that reports no usage; the model menu
//! dropping Compact and Clear when there is no ring, or keeping them beside
//! one.
//!
//! The menu rises into place over `MENU_IN` through `with_animation`, which
//! reads the wall clock, not the test executor's: advancing the test clock
//! does not finish it. Every geometry check reads the menu after it has come
//! to rest (`open` waits for that, with a bound), and compares rows only
//! within one frame.

use std::{cell::Cell, rc::Rc, time::Duration};

use web_time::Instant;

use gpui::{
    Bounds, Context, IntoElement, Modifiers, Pixels, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        context_window::{CompactState, ContextControls, ContextWindow, context_ring_shown},
        model_picker::{ModelPicker, default_providers},
    },
};

#[derive(Clone, Default)]
struct Counts {
    compacted: Rc<Cell<u32>>,
    cleared: Rc<Cell<u32>>,
}

struct Host {
    compact: CompactState,
    map: bool,
    counts: Counts,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let compacted = self.counts.compacted.clone();
        let cleared = self.counts.cleared.clone();
        div().size_full().pt(px(560.)).pl(px(120.)).child(
            ContextWindow::new("cw", "lumen-4", 200_000)
                .system(3_100)
                .tools(14_600)
                .conversation(96_300, 46)
                .compact(self.compact.clone())
                .window_map(self.map)
                .on_compact(move |_, _| compacted.set(compacted.get() + 1))
                .on_clear(move |_, _| cleared.set(cleared.get() + 1)),
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

fn open(cx: &mut VisualTestContext) -> (Bounds<Pixels>, Bounds<Pixels>) {
    let chip = cx.debug_bounds("cw-chip").expect("the ring");
    cx.simulate_click(chip.center(), Modifiers::none());
    settle(cx);
    (chip, at_rest(cx, "cw-menu"))
}

/// The wall-clock gap between the two frames `at_rest` compares. The rise
/// eases out, so it moves the menu less than the half-pixel layout grid over
/// this gap only in its last fraction of a pixel: two equal reads mean it has
/// arrived.
const REST_GAP: Duration = Duration::from_millis(50);
/// How long `at_rest` waits for the element to stop moving.
const REST_LIMIT: Duration = Duration::from_secs(2);

/// The element's bounds once two frames `REST_GAP` apart lay it out in the
/// same place; panics if it is still moving after `REST_LIMIT`.
fn at_rest(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    let deadline = Instant::now() + REST_LIMIT;
    let mut last = cx.debug_bounds(selector).expect(selector);
    loop {
        std::thread::sleep(REST_GAP);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let now = cx.debug_bounds(selector).expect(selector);
        if now == last {
            return now;
        }
        assert!(
            Instant::now() < deadline,
            "{selector} still moving after {REST_LIMIT:?}: {last:?} to {now:?}"
        );
        last = now;
    }
}

#[gpui::test]
fn the_menu_opens_from_the_rings_leading_edge(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        for map in [false, true] {
            let mut cx = cx.clone();
            let cx = mount(
                &mut cx,
                ui,
                Host {
                    compact: CompactState::Ready,
                    map,
                    counts: Counts::default(),
                },
            );
            let (chip, menu) = open(cx);
            let lead = f32::from(menu.origin.x) - f32::from(chip.origin.x);
            assert!(
                (lead + 4.0).abs() < 0.5,
                "the menu opens {lead} from the ring's leading edge at {ui}"
            );
            assert!(menu.bottom() <= chip.origin.y, "the menu covers the ring");
            if !map {
                assert_eq!(f32::from(menu.size.width), 280.0, "PINK's MENU_W_M");
            }
            for row in ["cw-compact", "cw-clear"] {
                let row = cx.debug_bounds(row).expect("a control row");
                assert!(row.origin.y >= menu.origin.y && row.bottom() <= menu.bottom());
            }
        }
    }
}

#[gpui::test]
fn arrows_wrap_and_enter_acts_on_the_highlighted_row(cx: &mut TestAppContext) {
    let counts = Counts::default();
    let cx = mount(
        cx,
        16.0,
        Host {
            compact: CompactState::Ready,
            map: false,
            counts: counts.clone(),
        },
    );
    open(cx);
    // Nothing is highlighted yet: Enter does nothing.
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!((counts.compacted.get(), counts.cleared.get()), (0, 0));
    // Down, Down, Down wraps from Clear back to Compact.
    cx.simulate_keystrokes("down down down enter");
    settle(cx);
    assert_eq!(counts.compacted.get(), 1, "Down wraps to Compact");
    // Clear asks once more before it acts.
    cx.simulate_keystrokes("down enter");
    settle(cx);
    assert_eq!(counts.cleared.get(), 0, "Clear acted on its first press");
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(counts.cleared.get(), 1, "the confirmed Clear");
    // Up, Up wraps back to Clear; Escape closes and disarms it.
    cx.simulate_keystrokes("up up enter escape");
    settle(cx);
    assert!(cx.debug_bounds("cw-menu").is_none(), "Escape closes");
    open(cx);
    cx.simulate_keystrokes("up enter");
    settle(cx);
    assert_eq!(counts.cleared.get(), 1, "the confirmation outlived Escape");
}

#[gpui::test]
fn compact_waits_when_the_provider_cant(cx: &mut TestAppContext) {
    for compact in [
        CompactState::Running,
        CompactState::Unknown,
        CompactState::Unavailable("This provider has no compact operation.".into()),
    ] {
        let counts = Counts::default();
        let mut cx = cx.clone();
        let cx = mount(
            &mut cx,
            16.0,
            Host {
                compact: compact.clone(),
                map: false,
                counts: counts.clone(),
            },
        );
        open(cx);
        let row = cx.debug_bounds("cw-compact").expect("Compact");
        cx.simulate_click(row.center(), Modifiers::none());
        cx.simulate_keystrokes("down enter");
        settle(cx);
        assert_eq!(counts.compacted.get(), 0, "{compact:?} compacted");
        // The reason, its row and the menu, all from the same frame.
        let row = cx.debug_bounds("cw-compact").expect("Compact");
        let menu = cx.debug_bounds("cw-menu").expect("the Context menu");
        let reason = cx.debug_bounds("cw-compact-reason");
        assert_eq!(
            reason.is_some(),
            matches!(compact, CompactState::Unavailable(_)),
            "{compact:?}: the reason shows only when Compact is unavailable"
        );
        if let Some(reason) = reason {
            assert!(reason.origin.y >= row.bottom() && reason.bottom() <= menu.bottom());
        }
    }
    assert_eq!(CompactState::Ready.label(), "Compact");
    assert_eq!(CompactState::Running.label(), "Compacting…");
    assert_eq!(CompactState::Unknown.label(), "Compact unresolved");
}

#[test]
fn the_ring_follows_the_setting_and_the_provider() {
    assert!(context_ring_shown(true, true));
    assert!(!context_ring_shown(true, false), "a provider without usage");
    assert!(!context_ring_shown(false, true), "the setting turned off");
    assert!(!context_ring_shown(false, false));
}

struct PickerHost {
    controls: bool,
}

impl Render for PickerHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let picker = ModelPicker::new("mp", default_providers()).default_open(true);
        div()
            .size_full()
            .pt(px(620.))
            .pl(px(40.))
            .child(if self.controls {
                picker.context_controls(ContextControls::new(CompactState::Ready))
            } else {
                picker
            })
    }
}

#[gpui::test]
fn the_model_menu_keeps_compact_and_clear_only_without_a_ring(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        for controls in [false, true] {
            let mut cx = cx.clone();
            cx.update(pink_ui::init);
            cx.update(|cx| {
                Theme::set_mode(ThemeMode::Light, cx);
                Theme::set_ui_font_size(ui, cx);
            });
            let (_, cx) = cx.add_window_view(|_, _| PickerHost { controls });
            settle(cx);
            let tray = cx.debug_bounds("mp-context");
            assert_eq!(tray.is_some(), controls, "the model menu's Context tray");
            if let Some(tray) = tray {
                for row in ["mp-context-compact", "mp-context-clear"] {
                    let row = cx.debug_bounds(row).expect("a tray row");
                    assert!(row.origin.y >= tray.origin.y && row.bottom() <= tray.bottom());
                }
            }
        }
    }
}
