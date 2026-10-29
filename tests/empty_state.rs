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
//! popup note whose padding stops growing with its line.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::empty_state::{
        ACTION_HEIGHT, ActionStyle, GUIDE_GLYPH, GUIDE_GLYPH_DROP, GUIDE_PAD_X, GuideStyle,
        NOTE_GAP, NOTE_PAD_Y, NoteStyle, PANE_MAX_WIDTH, PANE_PAD, PLACE_GLYPH, PLACE_NOTICE_WIDTH,
        POPUP_NOTE_PAD_Y, TILE, TileStyle, action, column, description, guide, guide_action,
        guide_line, guide_title, note, pane, place, place_glyph, place_message, place_notice,
        popup_note, shortcut, tile, title,
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
