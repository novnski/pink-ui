//! The composer's controls-row fit and access chip (Pink composer.rs
//! `controls_row_fit`, pickers.rs `access_chip_content`, PINK 0.30.4, UI
//! rethink D70). Failure modes: the access words dropped before the goal
//! label or the usage figure; the model's effort dropped before the access
//! words; a hidden goal or usage chip reported as collapsed; a row that
//! still overflows picked when a richer one fits; the access chip showing
//! only its glyph on a composer wide enough for its words, or keeping its
//! words on one too narrow; an existing thread's composer saying "Do
//! anything…".

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::chat_composer::{
        ChatComposer, ComposerModel, ControlsRowFit, ICON_BALANCED, ModelFit, controls_row_fit,
    },
};

const GAP: f32 = 4.0;

fn fit(budget: f32, goal: Option<(f32, f32)>, usage: Option<(f32, f32)>) -> ControlsRowFit {
    // Model 150 full, 110 without effort; access 120 in words, 28 glyph.
    controls_row_fit(budget, 150.0, 110.0, goal, usage, (120.0, 28.0), 1.0)
}

#[test]
fn pieces_yield_in_pinks_order() {
    let goal = Some((80.0, 28.0));
    let usage = Some((60.0, 28.0));
    let everything = 150.0 + GAP + 120.0 + GAP + 80.0 + GAP + 60.0;
    let full = fit(everything, goal, usage);
    assert_eq!(full.model, ModelFit::Full);
    assert!(!full.goal_icon_only && !full.usage_icon_only && !full.access_icon_only);
    // One point short: the goal label goes first.
    let a = fit(everything - 1.0, goal, usage);
    assert!(a.goal_icon_only && !a.usage_icon_only && !a.access_icon_only);
    assert_eq!(a.model, ModelFit::Full);
    // Then the usage figure.
    let b = fit(150.0 + GAP + 120.0 + GAP + 28.0 + GAP + 59.0, goal, usage);
    assert!(b.goal_icon_only && b.usage_icon_only && !b.access_icon_only);
    // Then the access words, while the model keeps its effort.
    let c = fit(150.0 + GAP + 119.0 + GAP + 28.0 + GAP + 28.0, goal, usage);
    assert!(c.access_icon_only);
    assert_eq!(c.model, ModelFit::Full);
    // Then the effort, then the name.
    let d = fit(149.0 + GAP + 28.0 + GAP + 28.0 + GAP + 28.0, goal, usage);
    assert_eq!(d.model, ModelFit::NameOnly);
    let e = fit(109.0 + GAP + 28.0 + GAP + 28.0 + GAP + 28.0, goal, usage);
    assert_eq!(e.model, ModelFit::IconOnly);
}

#[test]
fn absent_chips_are_never_collapsed() {
    let row = fit(150.0 + GAP + 119.0, None, None);
    assert!(row.access_icon_only);
    assert!(!row.goal_icon_only && !row.usage_icon_only);
}

struct Host {
    width: f32,
    in_thread: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            ChatComposer::new("cc")
                .width(self.width)
                .in_thread(self.in_thread)
                .models(vec![
                    ComposerModel::new("Opus 5.5", "Careful work", ICON_BALANCED).effort("Low"),
                ]),
        )
    }
}

fn access_words_shown(cx: &mut TestAppContext, ui: f32, width: f32) -> bool {
    cx.update(pink_ui::init);
    cx.update(|cx| {
        Theme::set_mode(ThemeMode::Dark, cx);
        Theme::set_ui_font_size(ui, cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| Host {
        width,
        in_thread: false,
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("cc-access").is_some(), "no access chip");
    cx.debug_bounds("cc-access-label").is_some()
}

#[gpui::test]
fn the_access_chip_names_its_mode_when_it_fits(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        assert!(
            access_words_shown(&mut cx.clone(), ui, 720.0),
            "wide at {ui}"
        );
        assert!(
            !access_words_shown(&mut cx.clone(), ui, 300.0),
            "narrow at {ui}"
        );
    }
}

#[test]
fn an_existing_thread_says_message_this_thread() {
    use pink_ui::components::chat_composer::placeholder_for;
    assert_eq!(placeholder_for(true, None), "Message this thread");
    assert_eq!(placeholder_for(false, None), "Do anything…");
    // A host's own placeholder wins either way.
    assert_eq!(
        placeholder_for(true, Some("Reply to Lena".into())),
        "Reply to Lena"
    );
}
