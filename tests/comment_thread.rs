//! The `pink-comment-thread` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out a review thread on its comment card band.
//! Failure modes: a thread whose header, meta rows, notes, plate or footer
//! stop following the analytic `thread_height` the diff's fold tween uses,
//! so a reply clips or a gap opens under the band; a note that leaves the
//! name's column; a long author name that pushes the time out of a narrow
//! card instead of truncating; a long plate label that overflows the band;
//! an avatar that stops scaling with the Interface size.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        comment_card::{self, ICON_COMMENT, posted_style},
        comment_thread::{
            ASSISTANT_MARK, AVATAR, AvatarStyle, ChipStyle, EDIT_GLYPH, LINE_HEIGHT, MessageLines,
            MetaStyle, PlateStyle, TEXT_INDENT, chip, count, count_label, footer, initial,
            initial_avatar, mark_avatar, message, messages, meta, note, plate, thread_height,
        },
    },
    pink::{MONO, Pink},
};

const LONG: &str = "Alexandria Konstantinopoulou-Vanderberg of the platform review group";

struct Host {
    ui: f32,
}

fn shape(lines: &[MessageLines]) -> Vec<MessageLines> {
    lines.to_vec()
}

const THREAD: [MessageLines; 3] = [
    MessageLines {
        lines: 1,
        plate: false,
    },
    MessageLines {
        lines: 2,
        plate: true,
    },
    MessageLines {
        lines: 1,
        plate: false,
    },
];

fn band(prefix: &'static str, width: f32, name: &'static str, t: &Pink) -> gpui::Div {
    let avatar = AvatarStyle {
        edge: t.hairline(0.12),
        fill: t.wash(0.08),
        content: t.text_faint,
    };
    let meta_style = MetaStyle {
        name: t.text,
        time: t.content_faint,
    };
    let plate_style = PlateStyle {
        background: t.ink(0.06),
        glyph: t.content_muted,
        text: t.content_muted,
    };
    div().w(px(width)).child(
        comment_card::band(&posted_style(t), None)
            .debug_selector(move || format!("{prefix}-band"))
            .child(
                comment_card::content(None)
                    .child(
                        comment_card::header(svg().path(ICON_COMMENT), t.text_faint)
                            .debug_selector(move || format!("{prefix}-header"))
                            .child(comment_card::location("src/lib.rs:14", t.text_faint, MONO))
                            .child(count(count_label(3), t.text_faint))
                            .child(chip(
                                svg().path(ICON_COMMENT),
                                "Resolved",
                                &ChipStyle {
                                    background: t.ink(0.06),
                                    text: t.content_muted,
                                },
                            )),
                    )
                    .child(
                        messages()
                            .child(
                                message()
                                    .child(
                                        meta(
                                            initial_avatar(initial(name), &avatar)
                                                .debug_selector(move || format!("{prefix}-avatar")),
                                            name,
                                            Some("3m".into()),
                                            &meta_style,
                                        )
                                        .debug_selector(move || format!("{prefix}-meta")),
                                    )
                                    .child(
                                        note(
                                            "Why clamp here?",
                                            t.text_dim,
                                            1,
                                        )
                                        .debug_selector(move || format!("{prefix}-body")),
                                    ),
                            )
                            .child(
                                message()
                                    .child(meta(
                                        mark_avatar(svg().path(ASSISTANT_MARK), &avatar),
                                        "Claude Code",
                                        Some("1m".into()),
                                        &meta_style,
                                    ))
                                    .child(note(
                                        "The clamp keeps the ratio inside 0..1.\nIt also guards the empty case.",
                                        t.text_dim,
                                        2,
                                    ))
                                    .child(
                                        plate(svg().path(EDIT_GLYPH), LONG, &plate_style)
                                            .debug_selector(move || format!("{prefix}-plate")),
                                    ),
                            )
                            .child(
                                message()
                                    .child(meta(
                                        initial_avatar("Y", &avatar),
                                        "You",
                                        Some("now".into()),
                                        &meta_style,
                                    ))
                                    .child(note(
                                        "Thanks.",
                                        t.text_dim,
                                        1,
                                    )),
                            ),
                    )
                    .child(
                        footer()
                            .child(div().w(px(40.0)).h(px(22.0))),
                    ),
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
            .child(band("wide", 480.0, "Sofia Marin", &t))
            .child(band("narrow", 220.0, LONG, &t))
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
fn comment_threads_follow_the_analytic_height_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let scale = ui / 16.0;
        let expected = thread_height(&shape(&THREAD), true, ui);

        for prefix in ["wide", "narrow"] {
            let band = bounds(cx, &format!("{prefix}-band"));
            assert!(
                near(band.size.height, expected),
                "{prefix} thread height at {ui}: {:?} vs {expected}",
                band.size.height
            );
            let avatar = bounds(cx, &format!("{prefix}-avatar"));
            assert!(
                near(avatar.size.width, AVATAR * scale),
                "{prefix} avatar scales at {ui}"
            );
            let body = bounds(cx, &format!("{prefix}-body"));
            assert!(
                near(body.origin.x - avatar.origin.x, TEXT_INDENT * scale),
                "{prefix} note sits under the name at {ui}"
            );
            assert!(
                near(body.size.height, LINE_HEIGHT * scale),
                "{prefix} note is its counted lines at {ui}"
            );
            let meta = bounds(cx, &format!("{prefix}-meta"));
            let plate = bounds(cx, &format!("{prefix}-plate"));
            assert!(
                meta.right() <= band.right() + px(0.01) && plate.right() <= band.right() + px(0.01),
                "{prefix} long name and plate stay inside the band at {ui}"
            );
        }
    }
}
