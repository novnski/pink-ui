//! The `pink-hunk-review` package at Interface sizes 12, 16 and 20, as PINK
//! lays its transcript diff's review bands out in rems, and the review model
//! PINK and the gallery share. Failure modes: a band whose painted height
//! drifts from `height(lines)` (PINK sums it analytically for the diff
//! viewport and the open tween, so rows would overlap or clip); padding,
//! header or status lines that stop scaling with the Interface size; actions
//! that leave the header and first status line's centre; a refusal that
//! replaces the recorded state, hides the engine's reason or still offers
//! the decision it refused while another one is in flight; Keep offered on a
//! kept block or Drop on a dropped one; accessible names that do not tell two
//! hunks apart; a modified key (Cmd-N, Ctrl-K) taken by the review; J and K
//! wrapping past the ends; the engine's `edit_review_*` code shown to the
//! reader.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    components::inline_diff::review::{
        self, BandStyle, BlockState, Command, Decision, KeyStyle, Status,
    },
    pink::Pink,
};

/// The band's pane width, in px: fixed while the Interface size changes.
const PANE: f32 = 560.0;

struct Host {
    ui: f32,
    status: Status,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = BandStyle {
            fill: t.ink(0.05),
            bar: t.solid,
            location: t.text_faint,
            headline: t.text_dim,
            detail: t.danger,
        };
        let keys = KeyStyle {
            cap: t.ink(0.06),
            key: t.text_muted,
            verb: t.text_muted,
        };
        let status = self.status.clone();
        div().flex().flex_col().items_start().child(
            div().w(gpui::px(PANE)).child(
                review::band(&style, status.lines())
                    .debug_selector(|| "band".into())
                    .child(review::bar(&style, 1.0).debug_selector(|| "bar".into()))
                    .child(
                        review::content()
                            .child(
                                review::header()
                                    .debug_selector(|| "header".into())
                                    .child(review::location_label(&style, "lib.rs:4"))
                                    .child(review::chip(t.text, t.ink(0.08), status.chip()))
                                    .child(
                                        review::legend().children(
                                            review::LEGEND.iter().map(|(k, verb)| {
                                                review::legend_item(&keys, k, verb)
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                review::headline(&style, status.headline())
                                    .debug_selector(|| "headline".into()),
                            )
                            .children(status.detail().map(|reason| {
                                review::detail(&style, reason).debug_selector(|| "detail".into())
                            })),
                    )
                    .child(
                        review::actions().child(
                            div()
                                .h(review::ui_rems(review::ACTION_HEIGHT))
                                .w(gpui::px(48.0))
                                .debug_selector(|| "action".into()),
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

fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 0.6, "{what}: {a} != {b}");
}

#[gpui::test]
fn bands_scale_with_the_interface_size_and_keep_their_analytic_height(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let refused = Status::Refused {
        state: BlockState::Pending,
        decision: Decision::Drop,
        reason: "Block or its recorded context changed or moved".into(),
    };
    for status in [Status::Settled(BlockState::Pending), refused] {
        for ui in [12.0_f32, 16.0, 20.0] {
            let scale = ui / 16.0;
            let status_for_host = status.clone();
            let (_, cx) = cx.add_window_view(move |_, _| Host {
                ui,
                status: status_for_host,
            });
            cx.run_until_parked();
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let band = bounds(cx, "band");
            let header = bounds(cx, "header");
            let headline = bounds(cx, "headline");
            let action = bounds(cx, "action");
            let bar = bounds(cx, "bar");
            let lines = status.lines();
            close(
                f32::from(band.size.height),
                review::height(lines) * scale,
                &format!("band height at {ui} with {lines} lines"),
            );
            close(f32::from(band.size.width), PANE, "band width");
            close(
                f32::from(header.origin.y - band.origin.y),
                review::PAD_Y * scale,
                "top padding",
            );
            close(
                f32::from(header.size.height),
                review::HEADER_HEIGHT * scale,
                "header height",
            );
            close(
                f32::from(headline.origin.y - header.origin.y),
                review::HEADER_HEIGHT * scale,
                "status line under the header",
            );
            close(
                f32::from(headline.size.height),
                review::LINE_HEIGHT * scale,
                "status line height",
            );
            // The actions sit on the header and first status line's centre.
            let pair_centre = f32::from(header.origin.y)
                + (review::HEADER_HEIGHT + review::LINE_HEIGHT) * scale / 2.0;
            close(
                f32::from(action.origin.y) + f32::from(action.size.height) / 2.0,
                pair_centre,
                "action centre",
            );
            close(
                f32::from(band.origin.x + band.size.width - action.origin.x - action.size.width),
                review::ACTIONS_INSET * scale,
                "actions inset",
            );
            // The bar is a hairline: fixed while the type grows.
            close(f32::from(bar.size.width), review::BAR_WIDTH, "bar width");
            if lines == 2 {
                let detail = bounds(cx, "detail");
                close(
                    f32::from(detail.origin.y - headline.origin.y),
                    review::LINE_HEIGHT * scale,
                    "reason under the headline",
                );
                assert!(
                    detail.origin.y + detail.size.height <= band.origin.y + band.size.height,
                    "the reason overflows its band at {ui}"
                );
            }
        }
    }
}

#[test]
fn a_refusal_keeps_the_recorded_state_and_names_the_reason() {
    let reason = review::refusal_reason(
        "rpc failed: edit_review_conflict: block or its recorded context changed or moved; newer content was preserved",
    );
    assert_eq!(
        reason.as_ref(),
        "Block or its recorded context changed or moved; newer content was preserved"
    );
    assert_eq!(
        review::refusal_reason("edit_review_busy: a provider is running in this checkout").as_ref(),
        "A provider is running in this checkout"
    );
    // A reason without an engine code reads as given.
    assert_eq!(
        review::refusal_reason("connection closed").as_ref(),
        "Connection closed"
    );
    let refused = Status::Refused {
        state: BlockState::Pending,
        decision: Decision::Drop,
        reason: reason.clone(),
    };
    // Nothing reads as dropped: the chip, tone and state stay the record's.
    assert_eq!(refused.state(), Some(BlockState::Pending));
    assert_eq!(refused.chip(), "Refused");
    assert_eq!(refused.tone(), review::Tone::Refused);
    assert_eq!(refused.detail(), Some(reason.clone()));
    assert!(refused.headline().contains("unchanged"));
    assert!(
        review::accessible_name("src/lib.rs", 4, &refused).contains(reason.as_ref()),
        "the band's name drops the reason"
    );
    // The refused drop can be retried, and Keep still settles it.
    assert!(refused.offers(Decision::Drop) && refused.offers(Decision::Keep));
    // In flight: no second decision, and the record still stands.
    let deciding = Status::Deciding {
        state: BlockState::Pending,
        decision: Decision::Drop,
    };
    assert_eq!(deciding.state(), Some(BlockState::Pending));
    assert!(!deciding.offers(Decision::Drop) && !deciding.offers(Decision::Keep));
    assert_ne!(deciding.chip(), "Dropped");
    assert!(!Status::Reading.offers(Decision::Keep));
}

#[test]
fn decisions_offered_follow_the_recorded_state() {
    let kept = Status::Settled(BlockState::Kept);
    let dropped = Status::Settled(BlockState::Dropped);
    let pending = Status::Settled(BlockState::Pending);
    assert!(!kept.offers(Decision::Keep) && kept.offers(Decision::Drop));
    // Keep on a dropped block is the undo.
    assert!(dropped.offers(Decision::Keep) && !dropped.offers(Decision::Drop));
    assert!(pending.offers(Decision::Keep) && pending.offers(Decision::Drop));
    assert_eq!(
        review::action_name(Decision::Keep, "src/lib.rs", 9, &dropped).as_ref(),
        "Keep the change at src/lib.rs line 9 again"
    );
    assert_eq!(
        review::progress(&[BlockState::Kept, BlockState::Pending, BlockState::Dropped]).as_ref(),
        "2 of 3 reviewed"
    );
}

#[test]
fn names_tell_hunks_apart_without_separators() {
    let pending = Status::Settled(BlockState::Pending);
    let first = review::accessible_name("src/lib.rs", 2, &pending);
    let second = review::accessible_name("src/lib.rs", 14, &pending);
    assert_ne!(first, second);
    assert_ne!(
        review::action_name(Decision::Drop, "src/lib.rs", 2, &pending),
        review::action_name(Decision::Drop, "src/lib.rs", 14, &pending)
    );
    assert_eq!(
        review::location("crates/ui/src/lib.rs", 14).as_ref(),
        "lib.rs:14"
    );
    for status in [
        Status::Reading,
        pending,
        Status::Settled(BlockState::Kept),
        Status::Settled(BlockState::Dropped),
        Status::Deciding {
            state: BlockState::Dropped,
            decision: Decision::Keep,
        },
    ] {
        for text in [
            status.headline().to_string(),
            review::accessible_name("src/lib.rs", 2, &status).to_string(),
        ] {
            assert!(
                !text.contains(" · ") && !text.contains(" -- "),
                "a separator in {text:?}"
            );
        }
    }
}

#[test]
fn keys_review_and_move_but_leave_modified_keys_to_the_app() {
    assert_eq!(
        review::key_command("y", false, false, false),
        Some(Command::Decide(Decision::Keep))
    );
    assert_eq!(
        review::key_command("n", false, false, false),
        Some(Command::Decide(Decision::Drop))
    );
    assert_eq!(
        review::key_command("j", false, false, false),
        Some(Command::Step(1))
    );
    assert_eq!(
        review::key_command("k", false, false, false),
        Some(Command::Step(-1))
    );
    assert_eq!(review::key_command("n", true, false, false), None);
    assert_eq!(review::key_command("k", false, true, false), None);
    assert_eq!(review::key_command("y", false, false, true), None);
    assert_eq!(review::key_command("x", false, false, false), None);
    assert_eq!(review::step(0, -1, 3), 0);
    assert_eq!(review::step(2, 1, 3), 2);
    assert_eq!(review::step(1, 1, 3), 2);
    assert_eq!(review::step(0, 1, 0), 0);
}
