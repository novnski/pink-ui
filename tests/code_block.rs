//! The `pink-code-block` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK frames a fenced code block. Failure modes: a header strip or
//! action that scales when PINK keeps the chrome fixed (the strip jumps
//! height between sizes and the code's first line moves); actions
//! crowding the frame's right edge or drifting apart; a Copy pill that
//! changes height when it grows its "Copied" label; a bare block that still
//! draws an empty strip above its code. For the code surface header over a
//! tool's output or inline diff: a strip that scales or loses its inset
//! when it changes weight, a Copy action off the shared right inset, and a
//! label-only (diff) strip whose long file name runs past the code's inset
//! instead of truncating there.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, SharedString, TestAppContext, Window, div,
    prelude::*, transparent_black,
};
use pink_ui::{
    Theme, ThemeMode,
    components::code_block::{
        ACTION_GAP, ACTION_ICON, ACTION_SIZE, COPY_ICON, HEADER_HEIGHT, HEADER_PAD_LEFT,
        HEADER_PAD_RIGHT, copy_action, frame, frame_style, header, icon_action, surface_header,
    },
    pink::Pink,
};

/// The test column's width.
const COLUMN: f32 = 400.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = frame_style(&t);
        let glyph = |size: f32| div().flex_none().size(gpui::px(size)).into_any_element();
        let block = |prefix: &'static str, language: Option<&'static str>, copied: bool| {
            let actions = language.map(|_| {
                vec![
                    icon_action(
                        format!("{prefix}-fit"),
                        glyph(ACTION_ICON),
                        transparent_black(),
                    )
                    .debug_selector(move || format!("{prefix}-fit"))
                    .into_any_element(),
                    copy_action(
                        format!("{prefix}-copy"),
                        glyph(COPY_ICON),
                        copied.then(|| SharedString::from("Copied")),
                        t.content_muted,
                        transparent_black(),
                    )
                    .debug_selector(move || format!("{prefix}-copy"))
                    .into_any_element(),
                ]
            });
            let head = header(
                language.map(SharedString::from),
                actions.unwrap_or_default(),
                style,
            )
            .map(|head| head.debug_selector(move || format!("{prefix}-header")));
            frame(prefix, head, style)
                .debug_selector(move || format!("{prefix}-frame"))
                .child(
                    div()
                        .debug_selector(move || format!("{prefix}-body"))
                        .h(gpui::px(40.0)),
                )
        };
        div()
            .w(gpui::px(COLUMN))
            .flex()
            .flex_col()
            .gap(gpui::px(8.0))
            .font_family(pink_ui::pink::SANS)
            .child(block("rest", Some("rust"), false))
            .child(block("copied", Some("rust"), true))
            .child(block("bare", None, false))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: String) -> Bounds<Pixels> {
    // Selectors are `'static`; the few names built per prefix live on.
    let name: &'static str = Box::leak(name.into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn the_chrome_stays_fixed_around_the_code_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut copy_widths = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Light, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for prefix in ["rest", "copied"] {
            let frame = bounds(cx, format!("{prefix}-frame"));
            let header = bounds(cx, format!("{prefix}-header"));
            let body = bounds(cx, format!("{prefix}-body"));
            let fit = bounds(cx, format!("{prefix}-fit"));
            let copy = bounds(cx, format!("{prefix}-copy"));

            // A fixed strip inside the 1 px edge (its rule drawn inside its
            // height), then the body.
            assert!(
                near(f32::from(header.origin.y - frame.origin.y), 1.0)
                    && near(f32::from(header.size.height), HEADER_HEIGHT),
                "{prefix} header at {ui}: {header:?}"
            );
            assert!(
                near(f32::from(body.origin.y - header.bottom()), 0.0),
                "{prefix} body under the rule at {ui}"
            );
            // Fixed actions, centred in the strip, inset from the right edge.
            assert!(
                near(f32::from(fit.size.width), ACTION_SIZE)
                    && near(f32::from(fit.size.height), ACTION_SIZE)
                    && near(f32::from(copy.size.height), ACTION_SIZE),
                "{prefix} actions at {ui}: {fit:?} {copy:?}"
            );
            assert!(
                near(
                    f32::from(fit.origin.y - header.origin.y),
                    (HEADER_HEIGHT - ACTION_SIZE) / 2.0
                ),
                "{prefix} actions centred at {ui}"
            );
            assert!(
                near(f32::from(copy.origin.x - fit.right()), ACTION_GAP)
                    && near(f32::from(header.right() - copy.right()), HEADER_PAD_RIGHT),
                "{prefix} action spacing at {ui}"
            );
            assert!(
                near(f32::from(header.size.width), COLUMN - 2.0),
                "{prefix} header spans the frame at {ui}"
            );
        }
        let rest = bounds(cx, "rest-copy".into());
        let copied = bounds(cx, "copied-copy".into());
        assert!(
            copied.size.width > rest.size.width,
            "Copied grows its pill at {ui}"
        );
        copy_widths.push(f32::from(copied.size.width));

        // No strip on a bare block: the body starts inside the edge.
        assert!(
            cx.debug_bounds("bare-header").is_none(),
            "a bare block draws no header at {ui}"
        );
        let bare = bounds(cx, "bare-frame".into());
        let bare_body = bounds(cx, "bare-body".into());
        assert!(
            near(f32::from(bare_body.origin.y - bare.origin.y), 1.0),
            "bare body at {ui}"
        );
    }
    // The "Copied" label grows with the Interface size.
    assert!(
        copy_widths[0] < copy_widths[2],
        "the Copied label scales: {copy_widths:?}"
    );
}

/// A host drawing PINK's tool code surfaces: an "Output" segment with its
/// Copy action, and an inline diff whose file name is too long to fit.
struct SurfaceHost {
    ui: f32,
}

impl Render for SurfaceHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = frame_style(&t);
        let copy = copy_action(
            "output-copy",
            div()
                .flex_none()
                .size(gpui::px(COPY_ICON))
                .into_any_element(),
            None,
            t.content_muted,
            transparent_black(),
        )
        .debug_selector(|| "output-copy".into());
        let output = surface_header(
            div()
                .debug_selector(|| "output-label".into())
                .child("Output"),
            vec![copy.into_any_element()],
            style,
        )
        .debug_selector(|| "output-header".into());
        let diff = surface_header(
            div()
                .debug_selector(|| "diff-label".into())
                .min_w_0()
                .truncate()
                .child("a-file-name-long-enough-to-outrun-any-column-it-sits-in.rs +3 -1"),
            Vec::new(),
            style,
        )
        .debug_selector(|| "diff-header".into());
        div()
            .w(gpui::px(COLUMN))
            .flex()
            .flex_col()
            .gap(gpui::px(8.0))
            .font_family(pink_ui::pink::SANS)
            .child(frame("output", Some(output), style).child(div().h(gpui::px(40.0))))
            .child(frame("diff", Some(diff), style).child(div().h(gpui::px(40.0))))
    }
}

#[gpui::test]
fn the_tool_surface_strip_keeps_the_code_insets_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut label_widths = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Light, cx));
        let (_, cx) = cx.add_window_view(|_, _| SurfaceHost { ui });
        cx.run_until_parked();

        let output = bounds(cx, "output-header".into());
        let label = bounds(cx, "output-label".into());
        let copy = bounds(cx, "output-copy".into());
        assert!(
            near(f32::from(output.size.height), HEADER_HEIGHT),
            "a fixed strip at {ui}: {output:?}"
        );
        assert!(
            near(f32::from(label.origin.x - output.origin.x), HEADER_PAD_LEFT),
            "the caption at the code's inset at {ui}"
        );
        assert!(
            near(f32::from(copy.size.height), ACTION_SIZE)
                && near(
                    f32::from(copy.origin.y - output.origin.y),
                    (HEADER_HEIGHT - ACTION_SIZE) / 2.0
                )
                && near(f32::from(output.right() - copy.right()), HEADER_PAD_RIGHT),
            "the Copy action centred on the shared right inset at {ui}: {copy:?}"
        );
        label_widths.push(f32::from(label.size.width));

        let diff = bounds(cx, "diff-header".into());
        let name = bounds(cx, "diff-label".into());
        assert!(
            near(f32::from(diff.size.height), HEADER_HEIGHT)
                && near(f32::from(name.origin.x - diff.origin.x), HEADER_PAD_LEFT),
            "the diff strip at {ui}: {diff:?}"
        );
        assert!(
            f32::from(diff.right() - name.right()) >= HEADER_PAD_LEFT - 0.5,
            "a long file name truncates at the code's inset at {ui}: {name:?} in {diff:?}"
        );
    }
    // The caption grows with the Interface size; the strip does not.
    assert!(
        label_widths[0] < label_widths[2],
        "the caption scales: {label_widths:?}"
    );
}
