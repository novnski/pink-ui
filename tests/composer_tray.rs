//! A narrow composer's controls tray (Pink composer.rs
//! `render_controls_tray`, `COMPACT_BUTTON`, `TRAY_CHIP_BUDGET`, the
//! compact `context_width`; PINK 0.31.4, novnski/pink#199), measured from
//! the laid-out composer at Interface sizes 12 and 20. Failure modes: a
//! wide row collapsing into the tray; a narrow thread's row keeping chips
//! that no longer fit; the branch and PR chips sharing the chips' row, so
//! PRs squeeze the branch to a sliver and the access mode to its glyph; PR
//! chips overlapping the branch or leaving the tray; the tray staying open
//! after Escape or a press outside it; a menu opened from the tray closing
//! the tray under it.

use std::time::Duration;

use gpui::{
    Bounds, Context, IntoElement, Modifiers, Pixels, Render, TestAppContext, VisualTestContext,
    Window, div, point, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::chat_composer::{ChatComposer, Checkout, PullRequest, PullRequestState},
};

struct Host {
    width: f32,
    prs: usize,
}

fn prs(count: usize) -> Vec<PullRequest> {
    (0..count)
        .map(|ix| {
            PullRequest::new(
                "acme/storefront",
                101 + ix as u32,
                "Retire the checkout copy test",
                if ix == 0 {
                    PullRequestState::Draft
                } else {
                    PullRequestState::Open
                },
            )
        })
        .collect()
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().pt(px(560.)).pl(px(60.)).child(
            ChatComposer::new("cc")
                .width(self.width)
                .in_thread(true)
                // A branch that fits beside its PRs in the tray (PINK's
                // replay): the old shared row cut it to 35% of 320 pt.
                .checkout(Checkout::branch("annual-billing"))
                .branches(["main", "annual-billing"])
                .pull_requests(prs(self.prs)),
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
        Theme::set_mode(ThemeMode::Light, cx);
        Theme::set_ui_font_size(ui, cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| host);
    settle(cx);
    cx
}

fn within(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.x >= outer.origin.x - px(0.5)
        && inner.right() <= outer.right() + px(0.5)
        && inner.origin.y >= outer.origin.y - px(0.5)
        && inner.bottom() <= outer.bottom() + px(0.5)
}

fn open_tray(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let button = cx.debug_bounds("cc-tray").expect("the tray button");
    cx.simulate_click(button.center(), Modifiers::none());
    settle(cx);
    cx.debug_bounds("cc-tray-card").expect("the open tray")
}

#[gpui::test]
fn a_wide_row_keeps_its_chips(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        let mut cx = cx.clone();
        let cx = mount(
            &mut cx,
            ui,
            Host {
                width: 720.0,
                prs: 2,
            },
        );
        assert!(cx.debug_bounds("cc-tray").is_none(), "a tray on a wide row");
        let controls = cx.debug_bounds("cc-controls").expect("the controls row");
        let access = cx.debug_bounds("cc-access").expect("the access chip");
        assert!(within(access, controls));
    }
}

#[gpui::test]
fn the_tray_gives_the_branch_and_prs_their_own_row(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        // The branch's natural label, alone on a roomy row.
        let mut wide = cx.clone();
        let wide = mount(
            &mut wide,
            ui,
            Host {
                width: 720.0,
                prs: 0,
            },
        );
        let natural = wide.debug_bounds("cc-checkout-label").expect("the label");
        // The tray's chips without PRs: PRs must not change their fit.
        let mut bare = cx.clone();
        let bare = mount(
            &mut bare,
            ui,
            Host {
                width: 300.0,
                prs: 0,
            },
        );
        open_tray(bare);
        let bare_words = bare.debug_bounds("cc-access-label").is_some();
        let bare_access = bare.debug_bounds("cc-access").expect("the access chip");

        let mut cx = cx.clone();
        let cx = mount(
            &mut cx,
            ui,
            Host {
                width: 300.0,
                prs: 2,
            },
        );
        let button = cx.debug_bounds("cc-tray").expect("a narrow thread's tray");
        let controls = cx.debug_bounds("cc-controls").expect("the controls row");
        assert!(within(button, controls), "the tray button left the row");
        assert!(
            cx.debug_bounds("cc-checkout").is_none(),
            "the branch stayed in a narrow row"
        );
        let card = open_tray(cx);
        assert!(
            card.bottom() <= button.origin.y,
            "the tray covers its button"
        );
        let access = cx.debug_bounds("cc-access").expect("the access chip");
        let branch = cx.debug_bounds("cc-checkout").expect("the branch chip");
        let label = cx.debug_bounds("cc-checkout-label").expect("the label");
        assert_eq!(
            cx.debug_bounds("cc-access-label").is_some(),
            bare_words,
            "PRs changed the access chip's words in the tray at {ui}"
        );
        assert_eq!(
            access.size.width, bare_access.size.width,
            "PRs squeezed the access chip in the tray at {ui}"
        );
        assert!(
            branch.origin.y >= access.bottom(),
            "the branch shares the chips' row at {ui}"
        );
        assert!(
            label.size.width >= natural.size.width - px(0.5),
            "the branch label shrank in the tray at {ui}"
        );
        let mut prev = branch.right();
        for n in [101, 102] {
            let pr = cx
                .debug_bounds(Box::leak(format!("cc-pr-{n}").into_boxed_str()))
                .expect("a PR chip");
            assert!(within(pr, card), "PR #{n} left the tray at {ui}");
            assert!(pr.origin.x >= prev, "PR #{n} overlaps at {ui}");
            prev = pr.right();
        }
        for chip in [access, branch] {
            assert!(within(chip, card), "a chip left the tray at {ui}");
        }
    }
}

#[gpui::test]
fn escape_and_an_outside_press_close_the_tray(cx: &mut TestAppContext) {
    let cx = mount(
        cx,
        16.0,
        Host {
            width: 300.0,
            prs: 1,
        },
    );
    open_tray(cx);
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert!(cx.debug_bounds("cc-tray-card").is_none(), "Escape");
    open_tray(cx);
    cx.simulate_click(point(px(8.), px(8.)), Modifiers::none());
    settle(cx);
    assert!(
        cx.debug_bounds("cc-tray-card").is_none(),
        "an outside press"
    );
    // A second press on the button closes the open tray.
    open_tray(cx);
    let button = cx.debug_bounds("cc-tray").expect("the tray button");
    cx.simulate_click(button.center(), Modifiers::none());
    settle(cx);
    assert!(
        cx.debug_bounds("cc-tray-card").is_none(),
        "the button again"
    );
}

#[gpui::test]
fn a_menu_from_the_tray_keeps_it_open(cx: &mut TestAppContext) {
    let cx = mount(
        cx,
        16.0,
        Host {
            width: 300.0,
            prs: 1,
        },
    );
    open_tray(cx);
    let model = cx.debug_bounds("cc-model").expect("the model chip");
    cx.simulate_click(model.center(), Modifiers::none());
    settle(cx);
    assert!(cx.debug_bounds("cc-tray-card").is_some(), "the tray closed");
    assert!(cx.debug_bounds("cc-model-menu").is_some(), "the model menu");
    // Escape closes the menu first, then the tray.
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert!(
        cx.debug_bounds("cc-tray-card").is_some(),
        "both closed at once"
    );
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert!(cx.debug_bounds("cc-tray-card").is_none());
}
