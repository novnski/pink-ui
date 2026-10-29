//! The `pink-menu-card` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays its menus out. Failure modes: row padding that stays fixed
//! while the text grows (cramped rows at large sizes); a card inset or row
//! gap that scales and pulls rows off the card's concentric corners; a
//! border that shifts the rows. Around the rows: a separator that stops at
//! the card's inset instead of running border to border, or that thickens
//! with the Interface size; a search field, heading, trailing section or
//! key cap whose padding stays fixed while its text grows; a heading that
//! loses its hair-space tracking.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::menu_card::{
        CARD_INSET, CardStyle, KEY_CAP_HEIGHT, KeyHintStyle, MENU_GAP, ROW_PAD_X, ROW_PAD_Y,
        RowStyle, SEARCH_MARGIN, SEARCH_PAD_X, SEARCH_PAD_Y, SECTION_SPACE, card, heading, key_cap,
        key_hint_text, row, search_frame, section, separator, tracked_upper, ui_rems,
    },
    pink::Pink,
};

/// The mark's side inside each row, in pt at the 16 px default.
const MARK: f32 = 16.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let item = |n: &'static str, mark: &'static str| {
            row(&RowStyle {
                text: t.text,
                fill: t.element_active,
            })
            .debug_selector(move || n.into())
            .child(
                div()
                    .debug_selector(move || mark.into())
                    .flex_none()
                    .size(ui_rems(MARK)),
            )
        };
        div().w(gpui::px(240.0)).child(
            card(&CardStyle {
                border: t.border,
                background: t.surface_raised,
                text: t.text,
                font_family: pink_ui::pink::SANS.into(),
                shadows: None,
            })
            .debug_selector(|| "card".into())
            .flex()
            .flex_col()
            .child(item("first", "first-mark"))
            .child(item("second", "second-mark")),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn rows_grow_with_the_interface_size_inside_a_fixed_card_inset(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let c = bounds(cx, "card");
        let first = bounds(cx, "first");
        let second = bounds(cx, "second");
        let mark = bounds(cx, "first-mark");

        // The 1 px border plus the inset, at every size.
        let inset = 1.0 + CARD_INSET;
        assert!(
            near(f32::from(first.origin.x - c.origin.x), inset),
            "row inset x at {ui}"
        );
        assert!(
            near(f32::from(first.origin.y - c.origin.y), inset),
            "row inset y at {ui}"
        );
        assert!(
            near(f32::from(c.right() - first.right()), inset),
            "row fills the card at {ui}"
        );
        assert!(
            near(f32::from(second.origin.y - first.bottom()), MENU_GAP),
            "row gap at {ui}"
        );

        // The row's padding grows with the Interface size.
        assert!(
            near(
                f32::from(first.size.height),
                (MARK + 2.0 * ROW_PAD_Y) * scale
            ),
            "row height at {ui}: {:?}",
            first.size
        );
        assert!(
            near(f32::from(mark.origin.x - first.origin.x), ROW_PAD_X * scale),
            "row padding at {ui}"
        );
    }
}

/// A card holding the chrome around the rows: the search field, a heading,
/// a row, a separator, a trailing section and a key legend.
struct Chrome {
    ui: f32,
}

impl Render for Chrome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let mark = |name: &'static str| {
            div()
                .debug_selector(move || name.into())
                .flex_none()
                .size(ui_rems(MARK))
        };
        let keys = KeyHintStyle {
            cap: t.element_active,
            text: t.text_muted,
            divider: t.border,
            mono: pink_ui::pink::MONO.into(),
        };
        div().w(gpui::px(240.0)).child(
            card(&CardStyle {
                border: t.border,
                background: t.surface_raised,
                text: t.text,
                font_family: pink_ui::pink::SANS.into(),
                shadows: None,
            })
            .debug_selector(|| "card".into())
            .flex()
            .flex_col()
            .child(
                search_frame(t.element_hover)
                    .debug_selector(|| "search".into())
                    .child(mark("search-input")),
            )
            .child(heading(t.text_muted, "Models").debug_selector(|| "heading".into()))
            .child(
                row(&RowStyle {
                    text: t.text,
                    fill: t.element_active,
                })
                .debug_selector(|| "row".into())
                .child(mark("row-mark")),
            )
            .child(separator(t.border).debug_selector(|| "separator".into()))
            .child(
                section(t.border)
                    .debug_selector(|| "section".into())
                    .child(mark("section-mark")),
            )
            .child(
                div()
                    .flex()
                    .child(key_cap(t.element_active).debug_selector(|| "key-cap".into()))
                    .child(key_hint_text(&keys, "esc", "Close")),
            ),
        )
    }
}

#[gpui::test]
fn chrome_grows_with_the_interface_size_and_rules_run_border_to_border(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut headings = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Chrome { ui });
        cx.run_until_parked();

        let c = bounds(cx, "card");
        let search = bounds(cx, "search");
        let input = bounds(cx, "search-input");
        let heading = bounds(cx, "heading");
        let row = bounds(cx, "row");
        let rule = bounds(cx, "separator");
        let section = bounds(cx, "section");
        let section_mark = bounds(cx, "section-mark");
        let cap = bounds(cx, "key-cap");

        // The search field's padding and the room under it scale.
        assert!(
            near(
                f32::from(input.origin.x - search.origin.x),
                SEARCH_PAD_X * scale
            ),
            "search padding x at {ui}"
        );
        assert!(
            near(
                f32::from(input.origin.y - search.origin.y),
                SEARCH_PAD_Y * scale
            ),
            "search padding y at {ui}"
        );
        assert!(
            near(
                f32::from(heading.origin.y - search.bottom()),
                SEARCH_MARGIN * scale + MENU_GAP
            ),
            "room under the search field at {ui}"
        );

        // The separator spans the card inside its 1 px border, 1 px thick,
        // `MENU_GAP` clear of the row above at every size.
        assert!(
            near(f32::from(rule.origin.x - c.origin.x), 1.0),
            "separator starts at the border at {ui}: {rule:?} in {c:?}"
        );
        assert!(
            near(f32::from(c.right() - rule.right()), 1.0),
            "separator ends at the border at {ui}"
        );
        assert!(
            near(f32::from(rule.size.height), 1.0),
            "separator thickness at {ui}"
        );
        assert!(
            near(f32::from(rule.origin.y - row.bottom()), MENU_GAP * 2.0),
            "separator margin at {ui}"
        );

        // The trailing section's rule sits `SECTION_SPACE` below the rows
        // before it and its rows `SECTION_SPACE` below the rule, scaled.
        assert!(
            near(
                f32::from(section.origin.y - rule.bottom()),
                MENU_GAP + SECTION_SPACE * scale + MENU_GAP
            ),
            "section margin at {ui}"
        );
        assert!(
            near(
                f32::from(section_mark.origin.y - section.origin.y),
                1.0 + SECTION_SPACE * scale
            ),
            "section padding at {ui}"
        );

        assert!(
            near(f32::from(cap.size.height), KEY_CAP_HEIGHT * scale),
            "key cap height at {ui}"
        );
        headings.push((scale, f32::from(heading.size.height)));
    }
    // The heading's padding and text grow together.
    let (_, base) = headings[1];
    for (scale, height) in headings {
        assert!(
            (height - base * scale).abs() <= 1.0,
            "heading height {height} at scale {scale}, {base} at 16"
        );
    }
}

#[test]
fn headings_are_uppercase_with_hair_space_tracking() {
    assert_eq!(tracked_upper("ab"), "A\u{200A}B");
    assert_eq!(
        tracked_upper("Question"),
        "Q\u{200A}U\u{200A}E\u{200A}S\u{200A}T\u{200A}I\u{200A}O\u{200A}N"
    );
    assert_eq!(tracked_upper(""), "");
}
