//! The `pink-surface-toolbar` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out its Files search toolbar. Failure modes: a row
//! that grows with the Interface size and stops lining up with the workbench
//! tab strip; a field that stops growing with its query's type (the query
//! clips) or stops taking the room the toggles leave; controls that drift
//! off the fixed 28 px squares and the 8 px inset.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        icon_button::{IconButtonStyle, icon_button},
        surface_toolbar::{
            CONTROL_GAP, CONTROL_RADIUS, CONTROL_SIZE, EDGE_INSET, FIELD_GLYPH, FIELD_HEIGHT,
            HEIGHT, control_glyph, field, field_glyph, field_input, toolbar,
        },
    },
};

const WIDTH: f32 = 300.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let black = gpui::black();
        let toggle = |id: &'static str| {
            icon_button(
                id,
                control_glyph(gpui::svg(), black).into_any_element(),
                &IconButtonStyle {
                    size: CONTROL_SIZE,
                    radius: CONTROL_RADIUS,
                    wash: gpui::transparent_black(),
                    enabled: true,
                },
            )
        };
        div().w(px(WIDTH)).child(
            toolbar()
                .debug_selector(|| "toolbar".into())
                .child(
                    field(gpui::white())
                        .debug_selector(|| "field".into())
                        .child(field_glyph(gpui::svg(), black).debug_selector(|| "glyph".into()))
                        .child(field_input(div().h(px(16.0))).debug_selector(|| "input".into())),
                )
                .child(toggle("text"))
                .child(toggle("eye")),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn toolbar_keeps_its_row_and_scales_its_field(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let row = bounds(cx, "toolbar");
        let field = bounds(cx, "field");
        let glyph = bounds(cx, "glyph");
        let input = bounds(cx, "input");
        let text = bounds(cx, "text");
        let eye = bounds(cx, "eye");

        assert!(
            near(f32::from(row.size.height), HEIGHT),
            "row at {ui}: {row:?}"
        );
        assert!(
            near(f32::from(field.size.height), FIELD_HEIGHT * scale),
            "field at {ui}: {field:?}"
        );
        assert!(
            near(f32::from(glyph.size.width), FIELD_GLYPH * scale),
            "glyph at {ui}: {glyph:?}"
        );
        assert!(
            near(f32::from(field.origin.x - row.origin.x), EDGE_INSET),
            "inset at {ui}: {field:?} in {row:?}"
        );
        for control in [text, eye] {
            assert!(
                near(f32::from(control.size.width), CONTROL_SIZE)
                    && near(f32::from(control.size.height), CONTROL_SIZE),
                "control at {ui}: {control:?}"
            );
        }
        // The field takes what the toggles leave: they sit at the far edge.
        assert!(
            near(
                f32::from(row.origin.x + row.size.width - (eye.origin.x + eye.size.width)),
                EDGE_INSET
            ) && near(
                f32::from(eye.origin.x - (text.origin.x + text.size.width)),
                CONTROL_GAP
            ) && near(
                f32::from(text.origin.x - (field.origin.x + field.size.width)),
                CONTROL_GAP
            ),
            "layout at {ui}: {field:?} {text:?} {eye:?}"
        );
        assert!(
            f32::from(input.origin.x) > f32::from(glyph.origin.x + glyph.size.width),
            "query after the glyph at {ui}: {input:?}"
        );
    }
}
