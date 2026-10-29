//! The `pink-empty-state` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays out its Browser surface's pane state and its sidebar's
//! empty list. Failure modes: a glyph tile or action that keeps its design
//! size while the setting changes (PINK's grow with it); a column that runs
//! wider than its 300 pt measure, or past a narrow pane instead of wrapping
//! its description; a column that is no longer centred in the pane; a list
//! note whose fixed list insets or line gap start scaling with its lines; a
//! right-rail place glyph that scales with the setting (PINK keeps it at
//! 22 px like the rail's chrome), a place whose parts leave its centre, or a
//! notice that runs past its 280 px measure instead of wrapping; a guide
//! glyph that keeps its design size or stops sitting 1 px down on its
//! line's first row, or a guide line that runs past the list's insets; a
//! popup note whose padding stops growing with its line. The surface state
//! (PINK's Files surface and file documents): a glyph that keeps its design
//! size, a state that stops taking its surface's free height or leaves its
//! centre, a secondary line that runs past its 280 pt measure or into the
//! fixed 12 px side padding instead of wrapping, an action slot that loses
//! its extra space; a surface message off its surface's centre; an error
//! note whose padding stops growing with its line.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::empty_state::{
        ACTION_HEIGHT, ActionStyle, ERROR_PAD, GUIDE_GLYPH, GUIDE_GLYPH_DROP, GUIDE_PAD_X,
        GuideStyle, NOTE_GAP, NOTE_PAD_Y, NoteStyle, PANE_MAX_WIDTH, PANE_PAD, PLACE_GLYPH,
        PLACE_NOTICE_WIDTH, POPUP_NOTE_PAD_Y, SURFACE_ACTION_SPACE, SURFACE_BODY_WIDTH,
        SURFACE_GAP, SURFACE_GLYPH, SURFACE_PAD_X, TILE, TileStyle, action, column, description,
        error_note, guide, guide_action, guide_line, guide_title, note, pane, place, place_glyph,
        place_message, place_notice, popup_note, shortcut, surface, surface_action, surface_body,
        surface_glyph, surface_glyph_slot, surface_message, surface_title, tile, title,
    },
    pink::Pink,
};

/// The surfaces' widths, in px: fixed while the Interface size changes.
const PANE_W: f32 = 400.0;
const PANE_H: f32 = 420.0;
const LIST_W: f32 = 260.0;
const PLACE_W: f32 = 420.0;
const PLACE_H: f32 = 320.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div().w(px(PANE_W)).h(px(PANE_H)).child(
                    pane().debug_selector(|| "pane".into()).child(
                        column()
                            .debug_selector(|| "column".into())
                            .child(
                                tile(&TileStyle {
                                    edge: t.border,
                                    fill: t.surface_raised,
                                })
                                .debug_selector(|| "tile".into()),
                            )
                            .child(title(t.text).child("Preview your work"))
                            .child(
                                description(t.text_muted)
                                    .debug_selector(|| "description".into())
                                    .child(
                                        "Preview your local app or keep a website beside your conversation. "
                                            .repeat(3),
                                    ),
                            )
                            .child(
                                action(&ActionStyle {
                                    edge: t.border,
                                    text: t.text,
                                })
                                .debug_selector(|| "action".into())
                                .child("Enter an address")
                                .child(shortcut(t.text_faint).child("⌘L")),
                            ),
                    ),
                ),
            )
            .child(
                div().w(px(LIST_W)).child(
                    note(
                        &NoteStyle {
                            title: t.text_muted,
                            hint: t.text_faint,
                        },
                        "No threads yet",
                        "Start one with ⌘N.",
                    )
                    .debug_selector(|| "note".into()),
                ),
            )
            .child(
                div().w(px(PLACE_W)).h(px(PLACE_H)).child(
                    place()
                        .debug_selector(|| "place".into())
                        .child(place_glyph(
                            svg()
                                .path("icons/empty-state/git-diff.svg")
                                .debug_selector(|| "place-glyph".into()),
                            t.text_faint,
                        ))
                        .child(place_message(t.text_muted).child("No terminals open."))
                        .child(
                            place_notice(t.text_muted)
                                .debug_selector(|| "place-notice".into())
                                .child("Saved terminals couldn’t be restored. ".repeat(6)),
                        ),
                ),
            )
            .child({
                let style = GuideStyle {
                    title: t.text,
                    text: t.text_muted,
                };
                div().w(px(LIST_W)).child(
                    guide(&style)
                        .debug_selector(|| "guide".into())
                        .child(guide_title(&style).child("No related chats yet"))
                        .child(
                            guide_line(
                                &style,
                                svg()
                                    .path("icons/empty-state/bot.svg")
                                    .debug_selector(|| "guide-glyph".into()),
                                "Fresh side chats don't carry this chat's conversation. ".repeat(3),
                            )
                            .debug_selector(|| "guide-line".into()),
                        )
                        .child(guide_action().child("Side chat with context")),
                )
            })
            .child(
                div().w(px(LIST_W)).child(
                    popup_note(t.text_muted, "No models found")
                        .debug_selector(|| "popup-note".into()),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

#[gpui::test]
fn empty_states_scale_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut notes = Vec::new();
    let mut popups = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let tile = bounds(cx, "tile");
        assert_eq!(
            (f32::from(tile.size.width), f32::from(tile.size.height)),
            (TILE * scale, TILE * scale),
            "tile at {ui}"
        );
        let action = bounds(cx, "action");
        assert_eq!(
            f32::from(action.size.height),
            ACTION_HEIGHT * scale,
            "action height at {ui}"
        );

        let pane = bounds(cx, "pane");
        assert_eq!(
            f32::from(pane.size.width),
            PANE_W,
            "pane fills its surface at {ui}"
        );
        let column = bounds(cx, "column");
        let measure = (PANE_MAX_WIDTH * scale).min(PANE_W - 2.0 * PANE_PAD * scale);
        assert!(
            near(f32::from(column.size.width), measure),
            "column keeps its measure inside the pane at {ui}: {} vs {measure}",
            f32::from(column.size.width)
        );
        let pane_mid = f32::from(pane.origin.x) + PANE_W / 2.0;
        let column_mid = f32::from(column.origin.x) + f32::from(column.size.width) / 2.0;
        assert!(near(column_mid, pane_mid), "column centred at {ui}");
        let description = bounds(cx, "description");
        assert!(
            f32::from(description.size.width) <= f32::from(column.size.width) + 0.5,
            "description wraps inside the column at {ui}"
        );

        let note = bounds(cx, "note");
        assert_eq!(
            f32::from(note.size.width),
            LIST_W,
            "note spans its list at {ui}"
        );
        // The two lines grow with the setting; the list insets and the gap
        // between the lines stay fixed.
        notes.push(f32::from(note.size.height) - 2.0 * NOTE_PAD_Y - NOTE_GAP);

        let place = bounds(cx, "place");
        let glyph = bounds(cx, "place-glyph");
        assert_eq!(
            (f32::from(glyph.size.width), f32::from(glyph.size.height)),
            (PLACE_GLYPH, PLACE_GLYPH),
            "place glyph keeps its fixed size at {ui}"
        );
        let place_mid = f32::from(place.origin.x) + PLACE_W / 2.0;
        let glyph_mid = f32::from(glyph.origin.x) + PLACE_GLYPH / 2.0;
        assert!(near(glyph_mid, place_mid), "place glyph centred at {ui}");
        let notice = bounds(cx, "place-notice");
        assert!(
            near(f32::from(notice.size.width), PLACE_NOTICE_WIDTH),
            "a long notice wraps at its measure at {ui}: {}",
            f32::from(notice.size.width)
        );
        let notice_mid = f32::from(notice.origin.x) + f32::from(notice.size.width) / 2.0;
        assert!(near(notice_mid, place_mid), "notice centred at {ui}");

        let guide = bounds(cx, "guide");
        let glyph = bounds(cx, "guide-glyph");
        let line = bounds(cx, "guide-line");
        assert!(
            near(f32::from(glyph.size.width), GUIDE_GLYPH * scale),
            "guide glyph scales at {ui}"
        );
        assert!(
            near(f32::from(glyph.origin.y - line.origin.y), GUIDE_GLYPH_DROP),
            "guide glyph sits {GUIDE_GLYPH_DROP} px down its line at {ui}"
        );
        assert!(
            near(f32::from(line.size.width), LIST_W - 2.0 * GUIDE_PAD_X)
                && near(f32::from(line.origin.x - guide.origin.x), GUIDE_PAD_X),
            "guide line wraps inside the fixed list insets at {ui}"
        );

        let popup = bounds(cx, "popup-note");
        popups.push((f32::from(popup.size.height), POPUP_NOTE_PAD_Y * scale));
    }
    // The popup note's padding grows with its line: its height minus the
    // scaled padding is one line, which scales too.
    let lines: Vec<f32> = popups.iter().map(|(h, pad)| h - 2.0 * pad).collect();
    assert!(
        (lines[0] / lines[1] - 0.75).abs() < 0.06 && (lines[2] / lines[1] - 1.25).abs() < 0.06,
        "the popup note's padding scales with its line: {popups:?}"
    );
    let (small, large) = (notes[0] / notes[1], notes[2] / notes[1]);
    assert!(
        (small - 0.75).abs() < 0.04 && (large - 1.25).abs() < 0.04,
        "only the note's lines scale, not its fixed insets: {notes:?}"
    );
}

/// The surfaces the surface shapes fill, in px: a sidebar wide enough for
/// the 280 pt measure at every size, and one too narrow for it.
const SIDEBAR_W: f32 = 420.0;
const NARROW_W: f32 = 200.0;
const SIDEBAR_H: f32 = 360.0;

struct SurfaceHost {
    ui: f32,
}

impl Render for SurfaceHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let long = "The workspace folder was moved or deleted. ".repeat(4);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .w(px(SIDEBAR_W))
                    .h(px(SIDEBAR_H))
                    .flex()
                    .flex_col()
                    .debug_selector(|| "sidebar".into())
                    .child(
                        surface()
                            .debug_selector(|| "surface".into())
                            .child(
                                surface_glyph_slot().child(surface_glyph(
                                    svg()
                                        .path("icons/empty-state/folder.svg")
                                        .debug_selector(|| "surface-glyph".into()),
                                    t.text_faint,
                                )),
                            )
                            .child(surface_title(t.text).child("Files couldn't load"))
                            .child(
                                surface_body(t.text_muted)
                                    .debug_selector(|| "surface-body".into())
                                    .child(long.clone()),
                            )
                            .child(
                                surface_action()
                                    .debug_selector(|| "surface-action".into())
                                    .child("Retry"),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(NARROW_W))
                    .h(px(SIDEBAR_H))
                    .flex()
                    .flex_col()
                    .child(
                        surface()
                            .child(surface_title(t.text).child("Files couldn't load"))
                            .child(
                                surface_body(t.text_muted)
                                    .debug_selector(|| "narrow-body".into())
                                    .child(long),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(SIDEBAR_W))
                    .h(px(SIDEBAR_H))
                    .flex()
                    .flex_col()
                    .debug_selector(|| "document".into())
                    .child(
                        surface_message(t.text_faint, "Loading file…")
                            .child(div().debug_selector(|| "message-probe".into())),
                    ),
            )
            .child(
                div()
                    .w(px(LIST_W))
                    .child(error_note(t.danger, "Offline.").debug_selector(|| "error-note".into())),
            )
    }
}

#[gpui::test]
fn surface_states_scale_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut errors = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| SurfaceHost { ui });
        cx.run_until_parked();

        let sidebar = bounds(cx, "sidebar");
        let state = bounds(cx, "surface");
        assert_eq!(
            (state.size.width, state.size.height),
            (sidebar.size.width, sidebar.size.height),
            "surface state takes its surface's free space at {ui}"
        );
        let glyph = bounds(cx, "surface-glyph");
        assert!(
            near(f32::from(glyph.size.width), SURFACE_GLYPH * scale),
            "surface glyph scales at {ui}: {}",
            f32::from(glyph.size.width)
        );
        let mid = f32::from(sidebar.origin.x) + SIDEBAR_W / 2.0;
        assert!(
            near(
                f32::from(glyph.origin.x) + f32::from(glyph.size.width) / 2.0,
                mid
            ),
            "surface glyph centred at {ui}"
        );
        let body = bounds(cx, "surface-body");
        assert!(
            near(f32::from(body.size.width), SURFACE_BODY_WIDTH * scale),
            "a long secondary line wraps at its measure at {ui}: {}",
            f32::from(body.size.width)
        );
        assert!(
            near(
                f32::from(body.origin.x) + f32::from(body.size.width) / 2.0,
                mid
            ),
            "secondary line centred at {ui}"
        );
        let action = bounds(cx, "surface-action");
        assert!(
            near(
                f32::from(action.origin.y - body.bottom()),
                (SURFACE_GAP + SURFACE_ACTION_SPACE) * scale
            ),
            "action sits its extra space under the secondary line at {ui}"
        );
        let narrow = bounds(cx, "narrow-body");
        assert!(
            near(f32::from(narrow.size.width), NARROW_W - 2.0 * SURFACE_PAD_X),
            "in a narrow surface the line wraps inside the fixed side padding at {ui}: {}",
            f32::from(narrow.size.width)
        );

        let document = bounds(cx, "document");
        let probe = bounds(cx, "message-probe");
        let document_mid = f32::from(document.origin.y) + f32::from(document.size.height) / 2.0;
        assert!(
            near(f32::from(probe.origin.y), document_mid),
            "surface message centred in its surface at {ui}"
        );

        let error = bounds(cx, "error-note");
        errors.push((f32::from(error.size.height), ERROR_PAD * scale));
    }
    // The error note's padding grows with its line.
    let lines: Vec<f32> = errors.iter().map(|(h, pad)| h - 2.0 * pad).collect();
    assert!(
        (lines[0] / lines[1] - 0.75).abs() < 0.06 && (lines[2] / lines[1] - 1.25).abs() < 0.06,
        "the error note's padding scales with its line: {errors:?}"
    );
}
