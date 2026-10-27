//! The `pink-list-row` package's geometry and signals, laid out at Interface
//! sizes 12, 16 and 20. Failure modes: the title moves when the row is
//! hovered; the title column differs between scopes (an indent); a selected
//! row's insets are unbalanced; a row under 24 pt at size 12; two-line rows
//! under 48 × scale; Running announced or styled as needing the user; two
//! signals sharing a spoken name.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::list_row::{ListRow, Signal, list_row, row_height, row_style},
    pink::Pink,
};

struct Host {
    scale: f32,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let style = row_style(&t, self.scale);
        let row = |title: &str, two: bool, selected: bool, hovered: bool| ListRow {
            secondary: two.then(|| div().child("main").into_any_element()),
            time: Some("2m".into()),
            status: Some(Signal::Running),
            actions: Some(div().w(gpui::px(48.0)).h(gpui::px(24.0)).into_any_element()),
            selected,
            hovered,
            ..ListRow::new(title.to_string())
        };
        div()
            .w(gpui::px(272.0))
            .flex()
            .flex_col()
            .child(list_row(
                "rest",
                row("Backfill invoice PDFs", false, false, false),
                &style,
            ))
            .child(list_row(
                "hover",
                row("Backfill invoice PDFs", false, false, true),
                &style,
            ))
            .child(list_row(
                "selected",
                row("Backfill invoice PDFs", false, true, false),
                &style,
            ))
            .child(list_row(
                "two",
                row("Backfill invoice PDFs", true, false, false),
                &style,
            ))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn rows_keep_one_title_column_and_their_heights(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| {
            Theme::set_mode(ThemeMode::Dark, cx);
            Theme::set_ui_font_size(ui, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Host { scale });
        cx.run_until_parked();
        let rest = bounds(cx, "rest-title");
        let hover = bounds(cx, "hover-title");
        let selected = bounds(cx, "selected-title");
        let two = bounds(cx, "two-title");
        // One title column, at rest, hovered, selected and on two lines.
        for other in [hover, selected, two] {
            assert_eq!(other.origin.x, rest.origin.x, "title moved at {ui}");
        }
        // Hover never changes the title's width: the corner is fixed.
        assert_eq!(
            hover.size.width, rest.size.width,
            "hover squeezed the title at {ui}"
        );
        assert_eq!(f32::from(rest.origin.x), 8.0, "title inset at {ui}");
        let corner = bounds(cx, "selected-corner");
        let right = 272.0 - f32::from(corner.origin.x + corner.size.width);
        assert_eq!(right, 8.0, "selected row's trailing inset at {ui}");
        assert_eq!(row_height(false, scale), (32.0 * scale).max(24.0));
        assert!(row_height(false, scale) >= 24.0, "one-line row at {ui}");
        assert!(
            row_height(true, scale) >= 48.0 * scale,
            "two-line row at {ui}"
        );
    }
}

#[test]
fn signals_stay_separate_and_named() {
    let all = [
        Signal::Running,
        Signal::WaitingForInput,
        Signal::NeedsApproval,
        Signal::Failed,
        Signal::Unread,
    ];
    assert!(!Signal::Running.needs_you());
    assert_eq!(Signal::Running.word(), None);
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a.spoken(), b.spoken());
        }
    }
    let row = ListRow {
        status: Some(Signal::NeedsApproval),
        ..ListRow::new("Retire expired coupon codes")
    };
    assert_eq!(
        row.accessible_name().as_ref(),
        "Retire expired coupon codes, Waiting for your approval"
    );
}
