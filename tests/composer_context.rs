//! An existing thread's branch chip and pull requests in the composer's
//! controls row (Pink pickers.rs `render_context_row`, composer.rs
//! `context_width`, change_requests.rs `composer_task_pull_request_chrome`,
//! UI rethink #174), measured from the laid-out elements at Interface sizes
//! 12 and 20. Failure modes: the branch chip truncated to make room for PRs
//! that fit their own share; PR chips overlapping the branch or leaving the
//! controls row; an existing thread's chips drawn above the pill instead of in the
//! controls row; a new thread's checkout moved into the controls row; the
//! branch menu ticking a row by branch name when two checkouts are both on
//! `main`, or missing the checkout over a trailing slash.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::chat_composer::{
        BranchRef, ChatComposer, Checkout, PullRequest, PullRequestState, ticked_branch,
    },
};

struct Host {
    in_thread: bool,
    prs: usize,
    width: f32,
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
        div().size_full().child(
            ChatComposer::new("cc")
                .width(self.width)
                .in_thread(self.in_thread)
                .checkout(Checkout::branch("annual-billing").worktree("storefront-2"))
                .pull_requests(prs(self.prs)),
        )
    }
}

struct Measured {
    pill: Bounds<Pixels>,
    controls: Bounds<Pixels>,
    branch: Bounds<Pixels>,
    branch_label: Bounds<Pixels>,
    prs: Vec<Bounds<Pixels>>,
}

fn layout(cx: &mut TestAppContext, ui: f32, host: Host) -> Measured {
    cx.update(pink_ui::init);
    cx.update(|cx| {
        Theme::set_mode(ThemeMode::Light, cx);
        Theme::set_ui_font_size(ui, cx);
    });
    let count = host.prs;
    let (_, cx) = cx.add_window_view(|_, _| host);
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    Measured {
        pill: cx.debug_bounds("cc-pill").expect("the pill"),
        controls: cx.debug_bounds("cc-controls").expect("the controls row"),
        branch: cx.debug_bounds("cc-checkout").expect("the branch chip"),
        branch_label: cx
            .debug_bounds("cc-checkout-label")
            .expect("the branch label"),
        prs: (0..count)
            .map(|ix| {
                cx.debug_bounds(Box::leak(format!("cc-pr-{}", 101 + ix).into_boxed_str()))
                    .expect("a PR chip")
            })
            .collect(),
    }
}

fn inside(inner: Bounds<Pixels>, outer: Bounds<Pixels>) -> bool {
    inner.origin.y >= outer.origin.y && inner.bottom() <= outer.bottom() + gpui::px(0.5)
}

#[gpui::test]
fn the_branch_keeps_its_name_beside_every_pr(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        // PRs within their own share leave the branch whole. Crowded: four
        // PRs outgrow it, and still neither overlap the branch nor leave the
        // row.
        for (width, count) in [(720.0, 2), (560.0, 4)] {
            let mut alone = cx.clone();
            let solo = layout(
                &mut alone,
                ui,
                Host {
                    in_thread: true,
                    prs: 0,
                    width,
                },
            );
            let mut crowded = cx.clone();
            let busy = layout(
                &mut crowded,
                ui,
                Host {
                    in_thread: true,
                    prs: count,
                    width,
                },
            );
            if count == 2 {
                assert!(
                    busy.branch_label.size.width >= solo.branch_label.size.width,
                    "the branch label shrank beside {count} PRs at {ui}"
                );
            }
            assert!(
                inside(busy.branch, busy.controls),
                "branch chip off the row"
            );
            let mut prev = busy.branch.right();
            for pr in &busy.prs {
                assert!(inside(*pr, busy.controls), "PR chip off the controls row");
                assert!(pr.origin.x >= prev, "PR chips overlap at {ui}");
                prev = pr.right();
            }
            if count == 2 {
                assert!(
                    prev <= busy.pill.right(),
                    "PR chips overflow the pill at {ui}"
                );
            }
        }
    }
}

#[gpui::test]
fn a_new_thread_keeps_its_checkout_above_the_pill(cx: &mut TestAppContext) {
    let new = layout(
        cx,
        16.0,
        Host {
            in_thread: false,
            prs: 1,
            width: 560.0,
        },
    );
    assert!(
        new.branch.bottom() <= new.pill.origin.y,
        "checkout not above"
    );
    assert!(new.prs[0].bottom() <= new.pill.origin.y, "PR not above");
}

#[test]
fn the_branch_menu_ticks_the_actual_checkout() {
    let rows = [
        BranchRef::new("main").current("/work/storefront"),
        BranchRef::new("main").worktree("/work/storefront-2"),
        BranchRef::new("annual-billing"),
    ];
    // Both checkouts are on main: the worktree the thread runs in is ticked.
    let checkout = Checkout::branch("main")
        .worktree("storefront-2")
        .path("/work/storefront-2/");
    assert_eq!(ticked_branch(&rows, Some(&checkout)), Some(1));
    let project = Checkout::branch("main").path("/work/storefront");
    assert_eq!(ticked_branch(&rows, Some(&project)), Some(0));
    // A checkout path no row runs in ticks nothing, not the first main.
    let elsewhere = Checkout::branch("main").path("/elsewhere");
    assert_eq!(ticked_branch(&rows, Some(&elsewhere)), None);
    // Without a path the branch name decides.
    let named = Checkout::branch("annual-billing");
    assert_eq!(ticked_branch(&rows, Some(&named)), Some(2));
}
