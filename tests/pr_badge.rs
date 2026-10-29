//! The `pink-pr-badge` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays its pull request badges out. Failure modes: a badge whose
//! height stays at its 16 px size beside text that grows; a composer chip
//! wider than the room PINK reserves for it beyond its texts
//! (`pull_request_chrome`), so the branch chip beside it is squeezed; a
//! badge that shrinks in a crowded row and cuts its number.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, SharedString, TestAppContext, Window, div,
    prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        chat_composer::pull_request_chrome,
        pr_badge::{BadgeStyle, COMPOSER_HEIGHT, SIDEBAR_HEIGHT, Surface, badge, glyph, number},
    },
    pink::{MONO, Pink},
};

const ICON: &str = "icons/git-actions/git-pull-request.svg";

struct Host {
    ui: f32,
    width: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = || BadgeStyle {
            fill: Some(t.success.opacity(0.08)),
            text: t.success,
        };
        // Each part is measured on its own, laid out as the chip lays it.
        let texts = div()
            .flex()
            .flex_row()
            .text_size(pink_ui::components::pr_badge::ui_rems(
                pink_ui::components::pr_badge::COMPOSER_TEXT,
            ))
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(number(MONO, SharedString::from("#171")).debug_selector(|| "number-text".into()))
            .child(div().child("Draft").debug_selector(|| "draft-text".into()));
        div()
            .w(px(self.width))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div().flex().flex_row().child(
                    badge(Surface::Sidebar, &style())
                        .debug_selector(|| "sidebar".into())
                        .child(number(MONO, SharedString::from("#171"))),
                ),
            )
            .child(
                div().flex().flex_row().child(
                    badge(Surface::Composer, &style())
                        .debug_selector(|| "composer".into())
                        .child(glyph(ICON, t.success))
                        .child(number(MONO, SharedString::from("#171"))),
                ),
            )
            .child(
                div().flex().flex_row().child(
                    badge(Surface::Composer, &style())
                        .debug_selector(|| "draft".into())
                        .child(glyph(ICON, t.success))
                        .child(number(MONO, SharedString::from("#171")))
                        .child("Draft"),
                ),
            )
            .child(texts)
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 1.0
}

#[gpui::test]
fn badges_grow_with_the_interface_size_and_fit_their_reserved_room(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui, width: 400.0 });
        cx.run_until_parked();

        let sidebar = bounds(cx, "sidebar");
        let composer = bounds(cx, "composer");
        assert!(
            near(f32::from(sidebar.size.height), SIDEBAR_HEIGHT * scale),
            "sidebar badge height at {ui}: {:?}",
            sidebar.size
        );
        assert!(
            near(f32::from(composer.size.height), COMPOSER_HEIGHT * scale),
            "composer chip height at {ui}: {:?}",
            composer.size
        );

        // The chip is its texts plus exactly the chrome PINK reserves (less
        // the context row's gap before the chip, which the row draws).
        let number_text = f32::from(bounds(cx, "number-text").size.width);
        let draft_text = f32::from(bounds(cx, "draft-text").size.width);
        let row_gap = 2.0 * scale;
        let draft = bounds(cx, "draft");
        assert!(
            near(
                f32::from(composer.size.width),
                number_text + pull_request_chrome(false, scale) - row_gap
            ),
            "composer chip width at {ui}: {:?} for a {number_text} number",
            composer.size
        );
        assert!(
            near(
                f32::from(draft.size.width),
                number_text + draft_text + pull_request_chrome(true, scale) - row_gap
            ),
            "draft chip width at {ui}: {:?}",
            draft.size
        );
    }
}

#[gpui::test]
fn a_crowded_row_does_not_shrink_a_badge(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
    let (_, roomy) = cx.add_window_view(|_, _| Host {
        ui: 16.0,
        width: 400.0,
    });
    roomy.run_until_parked();
    let natural = bounds(roomy, "draft");

    let (_, crowded) = cx.add_window_view(|_, _| Host {
        ui: 16.0,
        width: 24.0,
    });
    crowded.run_until_parked();
    let squeezed = bounds(crowded, "draft");
    assert!(
        near(
            f32::from(squeezed.size.width),
            f32::from(natural.size.width)
        ) && near(
            f32::from(squeezed.size.height),
            f32::from(natural.size.height)
        ),
        "the chip shrank in a 24 px row: {squeezed:?} vs {natural:?}"
    );
}
