//! Skeleton: the `pink-skeleton` package (`components/skeleton`), PINK's
//! pulsing skeletons as a single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The kit's `pink::loaders::skeleton_rows`,
//! `skeleton_menu_rows` and `skeleton_bar` draw with it on the shared pulse
//! clock; this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/skeleton/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, prelude::*, px};

use crate::{
    motion::{self, PINK_PULSE},
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "Skeleton";
pub const DESCRIPTION: &str = "PINK's loading skeletons as a package the app pins: full-width ghost rows for a list or settings page, shorter pill rows of varied length for a loading menu, and the ghost label a chip holds while its value resolves, each row breathing a beat behind the one above";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let phase = motion::pulse_phase(&PINK_PULSE, window, cx);
        let quiet = t.fill(FillTier::Quaternary);
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(16.0))
            .child(
                div()
                    .w(px(240.0))
                    .child(rows(3, quiet, phase, motion::pulse_wave)),
            )
            .child(
                pink::popover_card(&t)
                    .w(px(240.0))
                    .flex()
                    .flex_col()
                    .child(menu_rows(5, quiet, phase, motion::pulse_wave)),
            )
            .child(bar(
                56.0,
                t.fill(FillTier::Tertiary),
                phase,
                motion::pulse_wave,
            ))
    })
}
