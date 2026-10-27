//! The composer's notices (Pink composer.rs `ONLY_IMAGES` and its failure
//! notice, task_commands.rs `failure_message`, notice.rs `notice_chip`;
//! PINK 0.30.3 and 0.31.0). Failure modes: a non-image file accepted, or an
//! image refused over its extension's case; a Can't attach message that
//! doesn't name the file or count several; a refused action that hides its
//! cause behind the next step, doubles a full stop, or says nothing when
//! only one half is known; a dash in the offline notice; a notice drawn
//! without its message or without a named Copy message button.

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::chat_composer::{ChatComposer, ComposerNotice, attachable_image, failure_message},
};

#[test]
fn only_images_can_be_attached() {
    for name in [
        "shot.png",
        "Shot.PNG",
        "photo.jpeg",
        "a.jpg",
        "anim.gif",
        "x.webp",
        "logo.svg",
        "scan.TIFF",
        "old.bmp",
    ] {
        assert!(attachable_image(name), "{name} refused");
    }
    for name in ["notes.pdf", "data.csv", "README", "archive.png.zip"] {
        assert!(!attachable_image(name), "{name} accepted");
    }
}

#[test]
fn cant_attach_names_the_file_and_why() {
    let one = ComposerNotice::cant_attach(&["notes.pdf"]);
    assert_eq!(one.title, "Can't attach");
    assert_eq!(
        one.message,
        "notes.pdf isn't an image. Only images can be attached for now."
    );
    let many = ComposerNotice::cant_attach(&["a.pdf", "b.csv", "c.txt"]);
    assert_eq!(
        many.message,
        "3 files aren't images. Only images can be attached for now."
    );
}

#[test]
fn a_refused_action_leads_with_its_cause() {
    let fallback = "Review the current task state and retry.";
    assert_eq!(
        failure_message(
            Some("The worktree has uncommitted changes"),
            Some("Commit or stash them, then retry."),
            fallback
        ),
        "The worktree has uncommitted changes. Commit or stash them, then retry."
    );
    assert_eq!(
        failure_message(
            Some("The branch is checked out elsewhere."),
            Some("Pick another."),
            fallback
        ),
        "The branch is checked out elsewhere. Pick another."
    );
    assert_eq!(failure_message(Some(" Busy "), None, fallback), "Busy");
    assert_eq!(
        failure_message(Some(""), Some("Try again."), fallback),
        "Try again."
    );
    assert_eq!(failure_message(None, None, fallback), fallback);
    let notice = ComposerNotice::action_failed(Some("Busy"), Some("Wait."), fallback);
    assert_eq!(notice.title, "Action failed");
    assert_eq!(notice.message, "Busy. Wait.");
    assert_eq!(
        ComposerNotice::offline().message,
        "Offline. Messages will send when you're back online."
    );
}

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            ChatComposer::new("cc")
                .width(560.0)
                .notice(ComposerNotice::cant_attach(&["notes.pdf"]))
                .on_dismiss_notice(|_, _| {}),
        )
    }
}

#[gpui::test]
fn the_notice_sits_above_the_pill_with_its_controls(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| {
            Theme::set_mode(ThemeMode::Light, cx);
            Theme::set_ui_font_size(ui, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Host);
        cx.run_until_parked();
        let notice = cx.debug_bounds("cc-notice").expect("the notice");
        let message = cx.debug_bounds("cc-notice-message").expect("its message");
        let copy = cx
            .debug_bounds("cc-notice-copy")
            .expect("its Copy message button");
        let pill = cx.debug_bounds("cc-pill").expect("the pill");
        assert!(
            notice.bottom() <= pill.origin.y,
            "notice not above the pill"
        );
        assert!(
            message.origin.y >= copy.bottom(),
            "message not under the title row"
        );
    }
}
