//! The `pink-comment-card` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays out its review cards. Failure modes: a posted card whose
//! header or note lines stop following the analytic `card_height` the diff's
//! fold tween uses, so the note clips or a gap opens under it; an accent bar
//! that is not exactly the diff rows' bar column, so the card's edge steps in
//! and out of the diff; a column card whose content leaves the centred
//! document column or whose bar leaves the band's edge; a long location that
//! pushes edit and remove out of a narrow card instead of truncating; an
//! adder, edit or remove that grows with the Interface size; a band draft's
//! actions that scale, or a floating draft's that do not; a band draft whose
//! field and actions spill past its bottom edge into the next diff row.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::comment_card::{
        ACCENT_BAR_WIDTH, ACTION_HEIGHT, ADDER, ADDER_GLYPH, Column, EDIT, GAP, HEADER_HEIGHT,
        ICON_COMMENT, ICON_PEN, ICON_PLUS, ICON_REMOVE, LINE_HEIGHT, PAD_V, PAD_X, REMOVE_HIT,
        REMOVE_INSET, action, action_style, actions, adder, body, card, card_height, content,
        draft, draft_height, draft_style, edit, field, floating, header, location, posted_style,
        remove, scaled_action,
    },
    pink::{MONO, Pink},
};

const WIDTH: f32 = 480.0;
const NARROW: f32 = 200.0;
const COLUMN: Column = Column {
    max_width: 300.0,
    gutter: 24.0,
};

struct Host {
    ui: f32,
}

fn posted(
    prefix: &'static str,
    at: &'static str,
    note: &'static str,
    column: Option<Column>,
    ui: f32,
    t: &Pink,
) -> gpui::Div {
    let group = format!("{prefix}-group");
    card(
        &posted_style(t),
        card_height(note.lines().count(), ui),
        column,
    )
    .group(group.clone())
    .debug_selector(move || format!("{prefix}-band"))
    .child(
        content(column)
            .debug_selector(move || format!("{prefix}-content"))
            .child(
                header(svg().path(ICON_COMMENT), t.text_faint)
                    .debug_selector(move || format!("{prefix}-header"))
                    .child(location(at, t.text_faint, MONO))
                    .child(
                        edit(svg().path(ICON_PEN), t.text_muted, group.clone())
                            .debug_selector(move || format!("{prefix}-edit")),
                    )
                    .child(
                        remove(svg().path(ICON_REMOVE), t.text_muted, group)
                            .debug_selector(move || format!("{prefix}-remove")),
                    ),
            )
            .child(body(note, t.text_dim).debug_selector(move || format!("{prefix}-body"))),
    )
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let ui = self.ui;
        let t = Pink::get(cx);
        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .child(posted("one", "src/lib.rs:3", "One line.", None, ui, &t))
            .child(posted(
                "two",
                "docs/guide.md:3",
                "First line.\nSecond line.",
                Some(COLUMN),
                ui,
                &t,
            ))
            .child(div().w(px(NARROW)).flex().flex_col().child(posted(
                "narrow",
                "an/unusually/deep/path/to/a/file/name.rs:120",
                "Short.",
                None,
                ui,
                &t,
            )))
            .child(
                adder(
                    t.solid,
                    svg()
                        .path(ICON_PLUS)
                        .debug_selector(|| "adder-glyph".into()),
                    t.on_solid,
                )
                .debug_selector(|| "adder".into()),
            )
            .child(
                draft(&draft_style(&t), draft_height(ui), None)
                    .debug_selector(|| "draft".into())
                    .child(
                        content(None)
                            .child(header(svg().path(ICON_COMMENT), t.text_faint))
                            .child(field(div().child("Note")))
                            .child(
                                actions().debug_selector(|| "draft-actions".into()).child(
                                    action(&action_style("cancel", false, &t), "Cancel")
                                        .debug_selector(|| "action".into()),
                                ),
                            ),
                    ),
            )
            .child(
                floating(div().w(px(WIDTH)))
                    .debug_selector(|| "floating".into())
                    .child(
                        header(svg().path(ICON_COMMENT), t.text_faint)
                            .debug_selector(|| "floating-header".into()),
                    )
                    .child(
                        scaled_action(&action_style("save", true, &t), "Save")
                            .debug_selector(|| "scaled-action".into()),
                    ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() < 0.01
}

#[gpui::test]
fn comment_cards_follow_the_analytic_height_and_the_bar_column(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let scale = ui / 16.0;

        for (prefix, lines) in [("one", 1), ("two", 2), ("narrow", 1)] {
            let band = bounds(cx, &format!("{prefix}-band"));
            assert!(
                near(band.size.height, card_height(lines, ui)),
                "{prefix} band height at {ui}: {:?}",
                band.size.height
            );
            let header = bounds(cx, &format!("{prefix}-header"));
            assert!(
                near(header.size.height, HEADER_HEIGHT * scale),
                "{prefix} header at {ui}"
            );
            assert!(
                near(header.origin.y - band.origin.y, PAD_V / 2.0),
                "{prefix} header inset at {ui}"
            );
            let body = bounds(cx, &format!("{prefix}-body"));
            // The note fills the band to its padding: its lines plus the
            // fixed slack, as the analytic height promises.
            assert!(
                near(band.bottom() - body.bottom(), PAD_V / 2.0),
                "{prefix} note reaches the padding at {ui}"
            );
            assert!(
                near(body.size.height, lines as f32 * LINE_HEIGHT * scale + GAP),
                "{prefix} note height at {ui}: {:?}",
                body.size.height
            );
            let edit = bounds(cx, &format!("{prefix}-edit"));
            assert!(near(edit.size.width, EDIT), "{prefix} edit at {ui}");
            let remove = bounds(cx, &format!("{prefix}-remove"));
            assert!(
                near(remove.size.width, REMOVE_HIT),
                "{prefix} remove at {ui}"
            );
            // Remove's negative margin reaches past the row by its inset
            // and no further.
            assert!(
                remove.right() <= header.right() + px(REMOVE_INSET) + px(0.01),
                "{prefix} remove stays inside the card at {ui}: {:?} > {:?}",
                remove.right(),
                header.right()
            );
        }

        let one = bounds(cx, "one-band");
        let one_header = bounds(cx, "one-header");
        assert!(
            near(one_header.origin.x - one.origin.x, ACCENT_BAR_WIDTH + PAD_X),
            "full-width content after the bar and inset at {ui}"
        );

        let two = bounds(cx, "two-band");
        let two_content = bounds(cx, "two-content");
        assert!(
            near(two_content.size.width, COLUMN.max_width),
            "column content width at {ui}"
        );
        assert!(
            near(
                two_content.origin.x - two.origin.x,
                (WIDTH - COLUMN.max_width) / 2.0
            ),
            "column content centred at {ui}"
        );

        let adder = bounds(cx, "adder");
        assert!(near(adder.size.width, ADDER), "adder at {ui}");
        let glyph = bounds(cx, "adder-glyph");
        assert!(
            near(glyph.origin.x - adder.origin.x, (ADDER - ADDER_GLYPH) / 2.0),
            "adder plus centred at {ui}"
        );

        let draft = bounds(cx, "draft");
        assert!(
            near(draft.size.height, draft_height(ui)),
            "draft height at {ui}"
        );
        let draft_actions = bounds(cx, "draft-actions");
        assert!(
            near(draft.bottom() - draft_actions.bottom(), PAD_V / 2.0),
            "draft holds its field and actions at {ui}"
        );
        let action = bounds(cx, "action");
        assert!(
            near(action.size.height, ACTION_HEIGHT),
            "band action stays fixed at {ui}"
        );

        let floating = bounds(cx, "floating");
        let floating_header = bounds(cx, "floating-header");
        assert!(
            near(floating_header.origin.x - floating.origin.x, PAD_X),
            "floating inset at {ui}"
        );
        let scaled = bounds(cx, "scaled-action");
        assert!(
            near(scaled.size.height, ACTION_HEIGHT * scale),
            "floating action scales at {ui}"
        );
    }
}
