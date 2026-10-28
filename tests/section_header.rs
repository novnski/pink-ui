//! The `pink-section-header` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its sidebar headers out. Failure modes: a header
//! that shrinks under the 24 pt hit target at a small size or stops growing
//! with its text at a large one; a shelf label that drifts off the rows'
//! title column (a mark slot or gap that stops scaling); a trailing chevron
//! that loses its pulled-back bearing; a disclosure label that moves off
//! the fixed header padding.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::section_header::{
        CHEVRON_TRAILING_BEARING, HEADER_PAD_X, MARK_SLOT, MIN_HIT, SHELF_GAP, disclosure_header,
        disclosure_label, header_height, mark_slot, reveal_on_hover, shelf_count, shelf_header,
        shelf_label, trailing_chevron,
    },
    pink::Pink,
};

/// The chevron's side at the 16 px default.
const CHEVRON: f32 = 12.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let scale = self.ui / 16.0;
        let t = Pink::get(cx);
        let chevron = |name: &'static str| {
            div()
                .debug_selector(move || name.into())
                .size(gpui::rems(CHEVRON / 16.0))
        };
        div()
            .w(gpui::px(260.0))
            .flex()
            .flex_col()
            .child(
                disclosure_header(scale)
                    .debug_selector(|| "group".into())
                    .group("group")
                    .child(
                        disclosure_label(t.text, "Pinned").debug_selector(|| "group-label".into()),
                    )
                    .child(div().flex_1())
                    .child(reveal_on_hover("group").child(chevron("group-chevron"))),
            )
            .child(
                shelf_header(scale)
                    .debug_selector(|| "shelf".into())
                    .child(mark_slot().debug_selector(|| "shelf-mark".into()))
                    .child(shelf_label("Archived").debug_selector(|| "shelf-label".into()))
                    .child(shelf_count(t.text).child("48"))
                    .child(trailing_chevron().child(chevron("shelf-chevron"))),
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
fn headers_grow_with_the_interface_size_on_the_title_column(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let group = bounds(cx, "group");
        let label = bounds(cx, "group-label");
        let group_chevron = bounds(cx, "group-chevron");
        let shelf = bounds(cx, "shelf");
        let mark = bounds(cx, "shelf-mark");
        let shelf_text = bounds(cx, "shelf-label");
        let shelf_chevron = bounds(cx, "shelf-chevron");

        // Height: scaled with the text, never under the minimum hit.
        let height = header_height(scale);
        assert!(height >= MIN_HIT, "hit floor at {ui}");
        for (name, header) in [("group", group), ("shelf", shelf)] {
            assert!(
                near(f32::from(header.size.height), height),
                "{name} height at {ui}: {:?}",
                header.size
            );
        }

        // The disclosure label and chevron sit on the fixed padding.
        assert!(
            near(f32::from(label.origin.x - group.origin.x), HEADER_PAD_X),
            "group label at {ui}"
        );
        assert!(
            near(
                f32::from(group.right() - group_chevron.right()),
                HEADER_PAD_X
            ),
            "group chevron at {ui}"
        );

        // The shelf's label starts on the rows' title column.
        assert!(
            near(f32::from(mark.origin.x - shelf.origin.x), HEADER_PAD_X),
            "shelf mark at {ui}"
        );
        assert!(
            near(f32::from(mark.size.width), MARK_SLOT * scale),
            "mark slot at {ui}: {:?}",
            mark.size
        );
        assert!(
            near(
                f32::from(shelf_text.origin.x - shelf.origin.x),
                HEADER_PAD_X + (MARK_SLOT + SHELF_GAP) * scale
            ),
            "shelf title column at {ui}"
        );
        // The chevron's empty space is pulled back past the padding.
        assert!(
            near(
                f32::from(shelf.right() - shelf_chevron.right()),
                HEADER_PAD_X - CHEVRON_TRAILING_BEARING * scale
            ),
            "shelf chevron at {ui}"
        );
    }
}
