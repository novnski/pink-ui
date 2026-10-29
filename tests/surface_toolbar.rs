//! The `pink-surface-toolbar` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out its Files search toolbar. Failure modes: a row
//! that grows with the Interface size and stops lining up with the workbench
//! tab strip; a field that stops growing with its query's type (the query
//! clips) or stops taking the room the toggles leave; controls that drift
//! off the fixed 28 px squares and the 8 px inset. The editor header and
//! History's controls: a leaf crumb that shrinks before its parents (the
//! file name truncates while folders keep their room), a parent that
//! collapses under its floor, text controls and the compact search field
//! that grow taller with the Interface size (the row would stop holding
//! them), or a compact field squeezed under its narrowest.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        icon_button::{IconButtonStyle, icon_button},
        surface_toolbar::{
            ARTWORK, COMPACT_CLOSE, COMPACT_FIELD_MIN, CONTROL_GAP, CONTROL_RADIUS, CONTROL_SIZE,
            CRUMB_MIN, CrumbStyle, EDGE_INSET, FIELD_GLYPH, FIELD_HEIGHT, HEIGHT,
            LabeledControlStyle, TextControlStyle, breadcrumb, compact_close, compact_field,
            compact_field_input, compact_field_status, control_glyph, control_label, crumb,
            crumb_separator, editor_header, field, field_glyph, field_input, file_artwork,
            labeled_control, text_control, toolbar,
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

struct EditorHost {
    ui: f32,
}

/// An editor header `width` wide over a three-part path, tagged `tag`.
fn header(width: f32, tag: &'static str) -> gpui::Div {
    let black = gpui::black();
    let style = CrumbStyle {
        sans: "Inter".into(),
        leaf: black,
        parent: black,
    };
    let selector = |name: &'static str| -> &'static str {
        Box::leak(format!("{tag}-{name}").into_boxed_str())
    };
    let (row, artwork, parent, leaf, control) = (
        selector("row"),
        selector("artwork"),
        selector("parent"),
        selector("leaf"),
        selector("control"),
    );
    div().w(px(width)).child(
        editor_header()
            .debug_selector(move || row.into())
            .child(file_artwork(gpui::img("")).debug_selector(move || artwork.into()))
            .child(
                breadcrumb(format!("{tag}-crumbs"))
                    .child(
                        crumb("a-rather-long-parent-folder", false, &style)
                            .debug_selector(move || parent.into()),
                    )
                    .child(crumb_separator(gpui::svg(), black))
                    .child(crumb("index.html", true, &style).debug_selector(move || leaf.into())),
            )
            .child(
                text_control(
                    format!("{tag}-preview"),
                    &TextControlStyle {
                        sans: "Inter".into(),
                        text: black,
                        wash: gpui::transparent_black(),
                        enabled: true,
                    },
                )
                .debug_selector(move || control.into())
                .child(control_glyph(gpui::svg(), black))
                .child("Preview"),
            ),
    )
}

impl Render for EditorHost {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let black = gpui::black();
        div()
            .flex()
            .flex_col()
            .child(header(600.0, "wide"))
            .child(header(260.0, "narrow"))
            .child(
                div().w(px(220.0)).child(
                    toolbar()
                        .child(div().w(px(80.0)).flex_none().h(px(16.0)))
                        .child(
                            compact_field("search", 196.0, black)
                                .debug_selector(|| "search".into())
                                .child(compact_field_status(gpui::svg()))
                                .child(compact_field_input(div().h(px(14.0))))
                                .child(
                                    compact_close("close", black).debug_selector(|| "close".into()),
                                ),
                        )
                        .child(
                            labeled_control(
                                "fetch",
                                &LabeledControlStyle {
                                    wash: black,
                                    enabled: true,
                                },
                            )
                            .debug_selector(|| "fetch".into())
                            .child(control_glyph(gpui::svg(), black))
                            .child(control_label("Fetch all", black)),
                        ),
                ),
            )
    }
}

#[gpui::test]
fn editor_header_keeps_the_leaf_and_controls_hold_the_row(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| EditorHost { ui });
        cx.run_until_parked();

        let wide_row = bounds(cx, "wide-row");
        let artwork = bounds(cx, "wide-artwork");
        let wide_parent = bounds(cx, "wide-parent");
        let wide_leaf = bounds(cx, "wide-leaf");
        let wide_control = bounds(cx, "wide-control");
        let narrow_parent = bounds(cx, "narrow-parent");
        let narrow_leaf = bounds(cx, "narrow-leaf");
        let search = bounds(cx, "search");
        let close = bounds(cx, "close");
        let fetch = bounds(cx, "fetch");

        assert!(
            near(f32::from(wide_row.size.height), HEIGHT),
            "header at {ui}: {wide_row:?}"
        );
        assert!(
            near(f32::from(artwork.size.width), ARTWORK * scale),
            "artwork at {ui}: {artwork:?}"
        );
        assert!(
            near(f32::from(wide_control.size.height), CONTROL_SIZE)
                && near(
                    f32::from(
                        wide_row.origin.x + wide_row.size.width
                            - (wide_control.origin.x + wide_control.size.width)
                    ),
                    CONTROL_GAP
                ),
            "control at {ui}: {wide_control:?} in {wide_row:?}"
        );
        // Narrowed, the parent gives up its room; the leaf keeps all of it.
        assert!(
            near(
                f32::from(narrow_leaf.size.width),
                f32::from(wide_leaf.size.width)
            ),
            "leaf at {ui}: {narrow_leaf:?} vs {wide_leaf:?}"
        );
        assert!(
            f32::from(narrow_parent.size.width) < f32::from(wide_parent.size.width)
                && f32::from(narrow_parent.size.width) >= CRUMB_MIN * scale - 0.5,
            "parent at {ui}: {narrow_parent:?} vs {wide_parent:?}"
        );
        assert!(
            near(f32::from(search.size.height), CONTROL_SIZE)
                && near(f32::from(fetch.size.height), CONTROL_SIZE),
            "History controls at {ui}: {search:?} {fetch:?}"
        );
        assert!(
            f32::from(search.size.width) >= COMPACT_FIELD_MIN * scale - 0.5
                && f32::from(search.size.width) < 196.0,
            "squeezed search at {ui}: {search:?}"
        );
        assert!(
            near(f32::from(close.size.width), COMPACT_CLOSE * scale),
            "close at {ui}: {close:?}"
        );
    }
}
