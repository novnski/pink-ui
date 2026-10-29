//! Overlay Card: the `pink-overlay-card` package (`components/overlay-card`),
//! PINK's overlay cards as a single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The kit's `pink::tooltip_card`, `pink::TextTooltip`,
//! `pink::dialog_card` and `pink::stack_popup_card` draw with it through
//! [`tooltip_style`], [`dialog_style`] and [`stack_popup`]; this module adds
//! those and the story.

#[allow(dead_code)]
mod package {
    include!("../../components/overlay-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Window, div, prelude::*, px};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink, TextStyle, TypeStyleExt},
    story,
};

pub const NAME: &str = "Overlay Card";
pub const DESCRIPTION: &str = "PINK's overlay cards as a package the app pins: the opaque card every tooltip and hover card draws on, one line for a short label and wrapped for a long one, the dialog card a modal lays on its scrim with its faint amber warning, and the full-width popup that opens above the composer on the next step of its radius ladder";

/// A tooltip card's colours on Pink's theme (PINK popover.rs
/// `tooltip_card`): `border_strong`, the opaque `tooltip_surface()`,
/// `tooltip_shadows()`, and Geist named on the card.
pub fn tooltip_style(t: &Pink) -> TooltipStyle {
    TooltipStyle {
        border: t.border_strong,
        background: t.tooltip_surface(),
        shadows: t.tooltip_shadows(),
        font_family: pink::SANS.into(),
    }
}

/// A dialog card's colours on Pink's theme (PINK popover.rs `dialog_card`):
/// the popover plate, a `separator(false)` edge and `text`. The kit's plate
/// is opaque, so the card lifts with the frost rings over `shadow_lg`
/// (chained by `pink::dialog_card`) rather than the package's plain one.
pub fn dialog_style(t: &Pink) -> DialogStyle {
    DialogStyle {
        border: t.separator(false),
        background: t.surface_bg(),
        text: t.text,
        lifted: false,
    }
}

/// A dialog's warning paint (PINK popover.rs `modal_warning`): the amber
/// wash and edge, and `warning_muted` floored on the wash over the dialog
/// plate and the modal scrim.
pub fn modal_warning(t: &Pink) -> ModalWarning {
    let wash = warning_wash(t.warning);
    ModalWarning {
        wash,
        border: warning_border(t.warning),
        text: t.readable_on_stacks(
            t.warning_muted,
            &[&[t.scrim(pink::MODAL_SCRIM), t.surface_bg(), wash]],
            pink::TEXT_CONTRAST_TARGET,
        ),
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let tooltip = |label: &'static str| {
            text_tooltip(&tooltip_style(&t), t.text, label).type_style(TextStyle::Caption)
        };
        let popup_row = |key: &'static str, label: &'static str, active: bool| {
            pink::menu_row(key, active, &t)
                .rounded(px(STACK_POPUP_ROW_RADIUS))
                .child(label)
        };
        let warning = modal_warning(&t);
        div()
            .w(px(520.))
            .flex()
            .flex_col()
            .gap(px(28.))
            .text_color(t.text)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(16.))
                    .child(tooltip("Copy message"))
                    .child(tooltip(
                        "Approve for me: PINK runs edits and commands in this project without asking",
                    )),
            )
            .child(
                pink::modal_scrim(&t).h(px(260.)).rounded(px(12.)).child(
                    motion::dialog_in(
                        "overlay-card-dialog",
                        div().child(
                            pink::dialog_card(&t)
                                .child(pink::dialog_title("Send findings to Main?", &t))
                                .child(div().mt(px(6.)).child(pink::dialog_body(
                                    "Main gets the three findings as a note. Nothing runs until you send it a message.",
                                    &t,
                                )))
                                .child(
                                    warning_notice(&warning)
                                        .mt(px(12.))
                                        .px(px(10.))
                                        .py(px(8.))
                                        .rounded(px(8.))
                                        .text_size(px(12.))
                                        .child("Main changed since this chat forked."),
                                )
                                .child(
                                    div()
                                        .mt(px(16.))
                                        .flex()
                                        .flex_row()
                                        .justify_end()
                                        .gap(px(8.))
                                        .child(
                                            pink::button(
                                                "overlay-card-cancel",
                                                ButtonKind::Ghost,
                                                ControlSize::Lg,
                                                &t,
                                            )
                                            .child("Cancel"),
                                        )
                                        .child(
                                            pink::button(
                                                "overlay-card-send",
                                                ButtonKind::Primary,
                                                ControlSize::Lg,
                                                &t,
                                            )
                                            .child("Send findings"),
                                        ),
                                ),
                        ),
                    ),
                ),
            )
            .child(
                pink::stack_popup_card(&t)
                    .w_full()
                    .flex()
                    .flex_col()
                    .child(popup_row("overlay-card-popup-0", "/review", true))
                    .child(popup_row("overlay-card-popup-1", "/rewind", false))
                    .child(popup_row("overlay-card-popup-2", "/fork", false)),
            )
            .into_any_element()
    })
}
