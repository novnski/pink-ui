//! The `pink-empty-state` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays out its Browser surface's pane state and its sidebar's
//! empty list. Failure modes: a glyph tile or action that keeps its design
//! size while the setting changes (PINK's grow with it); a column that runs
//! wider than its 300 pt measure, or past a narrow pane instead of wrapping
//! its description; a column that is no longer centred in the pane; a list
//! note whose fixed list insets or line gap start scaling with its lines.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::empty_state::{
        ACTION_HEIGHT, ActionStyle, NOTE_GAP, NOTE_PAD_Y, NoteStyle, PANE_MAX_WIDTH, PANE_PAD,
        TILE, TileStyle, action, column, description, note, pane, shortcut, tile, title,
    },
    pink::Pink,
};

/// The surfaces' widths, in px: fixed while the Interface size changes.
const PANE_W: f32 = 400.0;
const PANE_H: f32 = 420.0;
const LIST_W: f32 = 260.0;

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
    }
    let (small, large) = (notes[0] / notes[1], notes[2] / notes[1]);
    assert!(
        (small - 0.75).abs() < 0.04 && (large - 1.25).abs() < 0.04,
        "only the note's lines scale, not its fixed insets: {notes:?}"
    );
}
