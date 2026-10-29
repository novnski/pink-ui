//! The `pink-recovery-card` package at Interface sizes 12, 16 and 20, as
//! PINK lays its timeline notices out in rems, and the recovery model PINK
//! and the gallery share. Failure modes: an action that keeps its 24 pt
//! height while the Interface size changes (PINK's notice buttons grow with
//! it); a detail that stops lining up with the title once the glyph or the
//! row gap scales; a card whose padding stops scaling; a headline with a
//! joiner or a fraction; an active recovery ignoring the run's error, an
//! exhausted one absorbing it, or a blank detail beating it; Send now
//! offered while the engine only observes or while a retry runs; Open
//! Providers losing the primary plate when there is no Send now; Cancel on
//! a running or exhausted recovery.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::recovery_card::{
        ACTION_HEIGHT, ActionStyle, CardAction, NoticeStyle, PAD_X, Recovery, RecoveryAction,
        action, card, detail, glyph_size, recovery_cause, title, title_row,
    },
    pink::Pink,
};

/// The card's pane width, in px: fixed while the Interface size changes.
const PANE: f32 = 420.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = NoticeStyle {
            tone: t.warning,
            glyph: t.warning,
            title: t.text,
            detail: t.content_muted,
        };
        div().flex().flex_col().items_start().child(
            div().w(gpui::px(PANE)).child(
                card(&style)
                    .debug_selector(|| "card".into())
                    .child(
                        title_row()
                            .child(
                                div()
                                    .size(glyph_size())
                                    .flex_none()
                                    .debug_selector(|| "glyph".into()),
                            )
                            .child(
                                title(&style, "Retrying in 5s, attempt 2 of 3")
                                    .debug_selector(|| "title".into()),
                            )
                            .child(
                                action(&ActionStyle {
                                    edge: t.border_strong,
                                    text: t.text,
                                })
                                .debug_selector(|| "action".into())
                                .child("Cancel"),
                            ),
                    )
                    .child(
                        detail(&style)
                            .debug_selector(|| "detail".into())
                            .child("The provider closed the stream before the turn finished."),
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
fn notice_parts_scale_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let action = bounds(cx, "action");
        assert_eq!(
            f32::from(action.size.height),
            ACTION_HEIGHT * scale,
            "action height at {ui}"
        );
        let card = bounds(cx, "card");
        assert_eq!(f32::from(card.size.width), PANE, "card width at {ui}");
        let glyph = bounds(cx, "glyph");
        assert!(
            (f32::from(glyph.origin.x - card.origin.x) - (1.0 + PAD_X * scale)).abs() < 0.01,
            "glyph inset at {ui}"
        );
        // The detail's text starts on the title's left edge at every size.
        let title = bounds(cx, "title");
        let detail = bounds(cx, "detail");
        let detail_text_x =
            f32::from(detail.origin.x) + pink_ui::components::recovery_card::DETAIL_INSET * scale;
        assert!(
            (detail_text_x - f32::from(title.origin.x)).abs() < 0.01,
            "detail lines up with the title at {ui}: {detail_text_x} vs {}",
            f32::from(title.origin.x)
        );
        assert!(
            title.origin.y + title.size.height <= detail.origin.y,
            "detail sits under the title row at {ui}"
        );
    }
}

fn labels(actions: &[CardAction]) -> Vec<(&'static str, bool)> {
    actions.iter().map(|a| (a.label, a.primary)).collect()
}

#[test]
fn one_card_names_its_cause_and_offers_the_right_actions() {
    let scheduled = Recovery::Scheduled {
        retry_in: 5,
        attempt: 2,
        of: 3,
    };
    assert_eq!(scheduled.headline(), "Retrying in 5s, attempt 2 of 3");
    assert_eq!(
        Recovery::WaitingForSignIn {
            machine: "Studio Mac".into()
        }
        .headline(),
        "Sign in on Studio Mac"
    );

    let error: gpui::SharedString = "The provider closed the stream.".into();
    let detail: gpui::SharedString = "Run interrupted by engine restart".into();
    let blank: gpui::SharedString = " ".into();
    assert_eq!(
        recovery_cause(&scheduled, None, Some(&error)),
        Some(error.clone())
    );
    assert_eq!(
        recovery_cause(&scheduled, Some(&detail), Some(&error)),
        Some(detail.clone())
    );
    assert_eq!(
        recovery_cause(&scheduled, Some(&blank), Some(&error)),
        Some(error.clone())
    );
    assert_eq!(
        recovery_cause(&Recovery::Exhausted, None, Some(&error)),
        None
    );

    assert_eq!(
        labels(&scheduled.actions(false, false)),
        [("Send now", true), ("Cancel", false)]
    );
    assert_eq!(
        labels(&Recovery::Held.actions(false, false)),
        [("Continue", true), ("Cancel", false)]
    );
    assert_eq!(labels(&Recovery::Running.actions(false, false)), []);
    assert_eq!(
        labels(&Recovery::CheckingDelivery.actions(false, false)),
        [("Cancel", false)]
    );
    // An expired sign-in the engine only observes: Open Providers is the
    // primary.
    let sign_in = Recovery::WaitingForSignIn {
        machine: "Studio Mac".into(),
    };
    assert_eq!(
        labels(&sign_in.actions(true, true)),
        [("Open Providers", true), ("Cancel", false)]
    );
    assert_eq!(
        labels(&sign_in.actions(false, true)),
        [
            ("Send now", true),
            ("Open Providers", false),
            ("Cancel", false)
        ]
    );
    assert!(Recovery::Exhausted.actions(false, false).is_empty());
    assert_eq!(
        Recovery::Held.actions(false, false)[0].action,
        RecoveryAction::SendNow
    );
}
