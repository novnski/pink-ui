//! Attachment Thumb: the `pink-attachment-thumb` package
//! (`components/attachment-thumb`), PINK's attachment thumbnails as a
//! single-GPUI package the app pins (UI rethink decision 54). The gallery
//! compiles the package's own source here, unchanged, against its GPUI;
//! PINK compiles the same file against its patched one. The Dropzone and
//! Image Lightbox stories draw their thumbnails with it; this module adds
//! only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/attachment-thumb/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, SharedString, Window, div, prelude::*, px, svg};

use crate::{
    motion,
    pink::{self, Pink, loaders},
    story,
};

pub const NAME: &str = "Attachment Thumb";
pub const DESCRIPTION: &str = "PINK's attachment thumbnails as a package the app pins: 112×80 tiles above a sent message, dimmed under an upload ring or spinner while their bytes cross, dashed when a file could not load and pulsing while one loads, and 56 pt squares staged above the composer with a remove disc that appears on hover";

/// Pink: `icons::CLOSE`.
const ICON_CLOSE: &str = "icons/image-lightbox/close.svg";
const IMAGE_CHECKOUT: &str = "images/image-lightbox/checkout-coupon-error.png";
const IMAGE_LATENCY: &str = "images/image-lightbox/p95-latency.png";

/// The sent tiles as PINK's transcript draws them (transcript.rs
/// `render_user_attachments`): loaded, crossing with a known percent,
/// crossing without one, missing and loading.
fn sent(pulse: f32, t: &Pink) -> impl IntoElement {
    let loaded = PlateStyle {
        edge: t.hairline(0.11),
        fill: t.ink(0.035),
    };
    let image_tile = |key: &'static str, source: &'static str| {
        plate(Kind::Sent, &loaded)
            .id(key)
            .cursor_pointer()
            .role(Role::Button)
            .aria_label("Preview image")
            .child(image(Kind::Sent, source))
    };
    sent_strip()
        .child(image_tile("story-thumb-sent", IMAGE_CHECKOUT))
        .child(
            image_tile("story-thumb-uploading", IMAGE_LATENCY)
                .child(sending(pulse).child(loaders::upload_progress_ring(62, RING))),
        )
        .child(
            image_tile("story-thumb-waiting", IMAGE_CHECKOUT).child(sending(pulse).child(
                loaders::mini_glyph_spinner("story-thumb-spinner", SPINNER_CELL, t.glyph),
            )),
        )
        .child(missing(
            Kind::Sent,
            &PlateStyle {
                edge: t.hairline(0.14),
                fill: t.ink(0.025),
            },
        ))
        .child(skeleton(
            Kind::Sent,
            &PlateStyle {
                edge: t.hairline(0.08),
                fill: t.ink(0.055),
            },
            pulse,
        ))
}

/// The staged squares as PINK's composer draws them (composer.rs
/// `render_attachment_strip`); hover one to show its remove disc.
fn staged(t: &Pink) -> impl IntoElement {
    let square = |ix: usize, source: &'static str| {
        let group = SharedString::from(format!("story-thumb-staged-{ix}"));
        div()
            .group(group.clone())
            .flex_none()
            .relative()
            .child(
                plate(
                    Kind::Staged,
                    &PlateStyle {
                        edge: t.hairline(0.10),
                        fill: gpui::transparent_black(),
                    },
                )
                .id(("story-thumb-staged", ix))
                .cursor_pointer()
                .child(image(Kind::Staged, source)),
            )
            .child(
                remove_disc(&RemoveStyle {
                    fill: t.surface_raised,
                    edge: t.border_strong,
                })
                .id(("story-thumb-remove", ix))
                .role(Role::Button)
                .aria_label("Remove attachment")
                .cursor_pointer()
                .group_hover(group, |s| s.opacity(1.0))
                .child(
                    svg()
                        .path(ICON_CLOSE)
                        .size(px(REMOVE_GLYPH))
                        .text_color(t.text),
                ),
            )
    };
    staged_strip()
        .child(square(0, IMAGE_CHECKOUT))
        .child(square(1, IMAGE_LATENCY))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let pulse = motion::pulse_wave(motion::pulse_phase(&motion::PINK_PULSE, window, cx));
        let t = Pink::get(cx);
        div()
            .w(px(640.0))
            .flex()
            .flex_col()
            .gap(px(24.0))
            .child(pink::card(&t).p(px(12.0)).child(sent(pulse, &t)))
            .child(pink::card(&t).pb(px(12.0)).child(staged(&t)))
    })
}
