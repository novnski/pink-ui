//! The `pink-prompt-library` package's geometry at Interface sizes 12, 16
//! and 20, as PINK's composer and the gallery's Prompt Library lay it out.
//! Failure modes: a blank chip taller than the line box it sits in (the
//! sentence's rhythm breaks and lines jump while typing); a blank that
//! ignores its measured width (the text field reflows on every key); a
//! repeated blank that stops matching the editable chip's height; a blank
//! count that stops growing with the Interface size; a narrow sentence that
//! splits a blank from the punctuation glued to it.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::prompt_library::{
        CHIP_INSET_Y, CHIP_PAD_X, ChipStyle, Part, atoms, blank_chip, blank_count, echo_chip,
        filled_palette, group, sentence,
    },
    pink::Pink,
};

const LINE: f32 = 22.75;
const BLANK_W: f32 = 64.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let scale = self.ui / 16.0;
        let line = px(LINE * scale);
        let style = filled_palette(t.accent, t.dark);
        let empty = ChipStyle {
            fill: t.text_muted,
            ink: t.text,
        };
        let parts = [
            Part::Text("Write to "),
            Part::Blank("name"),
            Part::Text(", then thank "),
            Part::Blank("name"),
            Part::Text("."),
        ];
        let mut first = true;
        let groups = atoms(parts).into_iter().enumerate().map(|(ix, atom)| {
            let mut row = group(line).debug_selector(move || format!("group-{ix}"));
            for piece in atom {
                row = match piece {
                    pink_ui::components::prompt_library::Piece::Word(word) => {
                        row.child(div().child(word))
                    }
                    pink_ui::components::prompt_library::Piece::Blank(_) if first => {
                        first = false;
                        row.child(
                            blank_chip(&empty, px(BLANK_W), line, px(CHIP_PAD_X))
                                .debug_selector(|| "blank".into())
                                .child(div().w(px(400.0)).child("a field wider than its chip")),
                        )
                    }
                    pink_ui::components::prompt_library::Piece::Blank(name) => row.child(
                        echo_chip(&style, name, line, px(CHIP_PAD_X))
                            .debug_selector(|| "echo".into()),
                    ),
                };
            }
            row
        });
        div()
            .w(px(360.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .child(div().flex_1().child("Customer follow-up"))
                    .child(blank_count(t.text_muted, 3).debug_selector(|| "count".into())),
            )
            .child(
                sentence(px(4.0))
                    .text_size(px(14.0 * scale))
                    .debug_selector(|| "sentence".into())
                    .children(groups),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn blanks_sit_inside_the_line_box_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut counts = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let line = LINE * scale;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let blank = bounds(cx, "blank");
        let echo = bounds(cx, "echo");
        let chip = line - 2.0 * CHIP_INSET_Y;
        assert!(
            (f32::from(blank.size.height) - chip).abs() < 0.5,
            "blank is {:?} tall at {ui}, expected {chip}",
            blank.size.height
        );
        assert!(
            (f32::from(echo.size.height) - chip).abs() < 0.5,
            "echo is {:?} tall at {ui}, expected {chip}",
            echo.size.height
        );
        assert!(
            (f32::from(blank.size.width) - BLANK_W).abs() < 0.5,
            "a wide field stretched its blank to {:?} at {ui}",
            blank.size.width
        );
        // The comma after the first blank stays in its group.
        let group = bounds(cx, "group-2");
        assert!(
            group.origin.x <= blank.origin.x
                && blank.origin.x + blank.size.width < group.origin.x + group.size.width,
            "the blank's punctuation left its group at {ui}"
        );
        counts.push(f32::from(bounds(cx, "count").size.height));
    }
    assert!(
        counts[0] < counts[1] && counts[1] < counts[2],
        "the blank count does not grow with the Interface size: {counts:?}"
    );
}
