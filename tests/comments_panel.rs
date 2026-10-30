//! The `pink-comments-panel` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out a document's review threads in its side pane.
//! Failure modes: an entry whose header, byline, note or replies line stop
//! following the analytic `entry_height`, so a fold clips the note or a gap
//! opens under the band; a pane title that drifts off the entries' text
//! column; a long author name that pushes the time out of a narrow pane
//! instead of truncating; a toolbar that stops holding the switch; a note
//! or byline that stops scaling with the Interface size.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        comment_card::{self, ICON_COMMENT, posted_style},
        comments_panel::{
            BYLINE_HEIGHT, LINE_HEIGHT, PaneStyle, SWITCH_WIDTH, TOOLBAR_HEIGHT, byline, content,
            count, entry, entry_height, heading, list, name, note, pane, replies, replies_label,
            time, toolbar,
        },
    },
    pink::{MONO, Pink, SANS},
};

const LONG: &str = "Alexandria Konstantinopoulou-Vanderberg of the platform review group";

struct Host {
    ui: f32,
}

/// One pane: the toolbar with a switch-sized slot, then two entries (a
/// two-line note with replies, a one-line note without).
fn side_pane(prefix: &'static str, width: f32, author: &'static str, t: &Pink) -> gpui::Div {
    let entry_band = |key: &'static str, lines: usize, reply_count: usize| {
        entry().child(
            comment_card::band(&posted_style(t), None)
                .debug_selector(move || format!("{prefix}-{key}-band"))
                .child(
                    content()
                        .debug_selector(move || format!("{prefix}-{key}-content"))
                        .child(
                            comment_card::header(svg().path(ICON_COMMENT), t.text_faint)
                                .child(comment_card::location("src/lib.rs:14", t.text_faint, MONO)),
                        )
                        .child(
                            byline()
                                .debug_selector(move || format!("{prefix}-{key}-byline"))
                                .child(name(author, t.text))
                                .child(
                                    time("12m", t.content_faint)
                                        .debug_selector(move || format!("{prefix}-{key}-time")),
                                ),
                        )
                        .child(
                            note(
                                "Why clamp here?\nThe ratio is never negative.",
                                t.text_dim,
                                lines,
                            )
                            .debug_selector(move || format!("{prefix}-{key}-note")),
                        )
                        .when(reply_count > 0, |el| {
                            el.child(replies(replies_label(reply_count), t.text_muted))
                        }),
                ),
        )
    };
    div().w(px(width)).h(px(400.0)).child(
        pane(&PaneStyle {
            background: t.bg,
            text: t.text,
            font_family: SANS.into(),
        })
        .child(
            toolbar()
                .debug_selector(move || format!("{prefix}-toolbar"))
                .child(
                    heading("Comments", t.text)
                        .debug_selector(move || format!("{prefix}-heading"))
                        .child(count("2", t.text_faint)),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(SWITCH_WIDTH))
                        .h(px(26.0))
                        .debug_selector(move || format!("{prefix}-switch")),
                ),
        )
        .child(
            list()
                .child(entry_band("first", 2, 3))
                .child(entry_band("second", 1, 0)),
        ),
    )
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(side_pane("wide", 420.0, "You", &t))
            .child(side_pane("narrow", 240.0, LONG, &t))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() < 0.51
}

#[gpui::test]
fn comments_panel_entries_follow_the_analytic_height_at_every_interface_size(
    cx: &mut TestAppContext,
) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let scale = ui / 16.0;
        for prefix in ["wide", "narrow"] {
            let toolbar = bounds(cx, &format!("{prefix}-toolbar"));
            assert!(
                near(toolbar.size.height, TOOLBAR_HEIGHT),
                "{prefix} toolbar row at {ui}"
            );
            let switch = bounds(cx, &format!("{prefix}-switch"));
            assert!(
                near(switch.size.width, SWITCH_WIDTH) && switch.right() <= toolbar.right(),
                "{prefix} switch keeps its width inside the toolbar at {ui}"
            );
            for (key, lines, with_replies) in [("first", 2, true), ("second", 1, false)] {
                let band = bounds(cx, &format!("{prefix}-{key}-band"));
                let expected = entry_height(lines, with_replies, ui);
                assert!(
                    near(band.size.height, expected),
                    "{prefix} {key} entry height at {ui}: {:?} vs {expected}",
                    band.size.height
                );
                let note = bounds(cx, &format!("{prefix}-{key}-note"));
                assert!(
                    near(note.size.height, lines as f32 * LINE_HEIGHT * scale),
                    "{prefix} {key} note is its counted lines at {ui}"
                );
                let byline = bounds(cx, &format!("{prefix}-{key}-byline"));
                assert!(
                    near(byline.size.height, BYLINE_HEIGHT * scale),
                    "{prefix} {key} byline scales at {ui}"
                );
                let time = bounds(cx, &format!("{prefix}-{key}-time"));
                assert!(
                    time.right() <= band.right() + px(0.01),
                    "{prefix} {key} long name keeps the time inside the band at {ui}"
                );
            }
            // The title starts on the entries' text column.
            let heading = bounds(cx, &format!("{prefix}-heading"));
            let note = bounds(cx, &format!("{prefix}-first-note"));
            let title_x = heading.origin.x + px(pink_ui::components::comments_panel::TITLE_INSET);
            assert!(
                near(title_x - note.origin.x, 0.0),
                "{prefix} title lines up with the entries' text at {ui}: {title_x:?} vs {:?}",
                note.origin.x
            );
        }
    }
}
