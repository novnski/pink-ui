//! Settings Parts: the `pink-settings-parts` package
//! (`components/settings-parts`), PINK's settings row tile, meta line,
//! option cards and error and warning strips as a single-GPUI package the
//! app pins (UI rethink decision 54). The gallery compiles the package's own
//! source here, unchanged, against its GPUI; PINK compiles the same file
//! against its patched one. The kit's `pink::row_tile` and `pink::meta_line`,
//! the Appearance Picker's theme cards, the Settings Page's warning strip and
//! the Dropzone's error strip draw through it. This module adds only the
//! story.

#[allow(dead_code)]
mod package {
    include!("../../components/settings-parts/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, SharedString, Window, div, prelude::*, px};

use crate::{
    components::settings_section as section,
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "Settings Parts";
pub const DESCRIPTION: &str = "PINK's settings row parts as a package the app pins: the identity tile at a row's leading edge, the quiet facts line under a row title, the row of preview cards for picking a visual option, and the red error and amber warning strips a card shows above its rows";

const ICON_TILE: &str = "icons/settings-page/monitor.svg";
const ICON_DANGER: &str = "icons/settings-page/danger-triangle.svg";
const ICON_SYSTEM: &str = "icons/appearance-picker/monitor.svg";
const ICON_LIGHT: &str = "icons/appearance-picker/sun.svg";
const ICON_DARK: &str = "icons/appearance-picker/moon.svg";

/// The kit's tile style: a 1 px `border`, `fill(Quaternary)`, the icon in
/// `text_muted`.
pub fn tile_style(t: &Pink) -> TileStyle {
    TileStyle {
        border: t.border,
        background: t.fill(FillTier::Quaternary),
        icon: t.text_muted,
    }
}

/// The kit's strip style for `tone` (danger, warning) and its readable
/// `muted` partner: the tone at 20% edge over a 6% plate, the copy in
/// `readable_tone(muted, 0.9)`.
pub fn strip_style(tone: gpui::Hsla, muted: gpui::Hsla, t: &Pink) -> StripStyle {
    let (plate, edge, tone) = pink::strip_colors(tone, muted, t);
    StripStyle { edge, plate, tone }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let muted = t.readable_muted(0.85);
        let fragment = |copy: &'static str| SharedString::from(copy).into_any_element();
        let preview = |bg: gpui::Hsla| {
            div()
                .size_full()
                .rounded(px(OPTION_CARD_RADIUS))
                .bg(bg)
                .into_any_element()
        };
        let card_style = OptionCardStyle {
            ring: t.accent,
            border: t.border,
            text: t.text,
            text_muted: t.text_muted,
        };
        div()
            .w(px(560.0))
            .flex()
            .flex_col()
            .child(
                section::section_card(&section::CardStyle {
                    border: t.border,
                    background: t.card_glass_bg(),
                })
                .mt(px(0.0))
                .child(
                    section::section_header().child(
                        section::header_text()
                            .child(section::header_title(t.text, "Devices"))
                            .child(section::row_description(
                                muted,
                                "Computers paired with this PINK",
                            )),
                    ),
                )
                .child(
                    section::card_row(Some(t.border))
                        .child(row_tile(&tile_style(&t), ICON_TILE))
                        .child(
                            section::row_text()
                                .child(section::row_title(t.text, "Studio Mac"))
                                .child(meta_line(
                                    t.readable_muted(0.8),
                                    [
                                        fragment("This Mac"),
                                        fragment("macOS 26"),
                                        fragment("Seen 2 minutes ago"),
                                    ],
                                )),
                        ),
                )
                .child(
                    section::card_row(Some(t.border)).child(
                        section::row_block()
                            .child(
                                error_strip(
                                    &strip_style(t.danger, t.danger_muted, &t),
                                    ICON_DANGER,
                                    "Pairing failed: the other device stopped answering",
                                )
                                .mt(px(0.0))
                                .w_full(),
                            )
                            .child(
                                warning_strip(
                                    &strip_style(t.warning, t.warning_muted, &t),
                                    ICON_DANGER,
                                    "This device has not synced for a week",
                                )
                                .mt(px(0.0))
                                .w_full(),
                            ),
                    ),
                ),
            )
            .child(
                div().mt(px(24.0)).child(
                    option_card_row()
                        .child(option_card(
                            &card_style,
                            ICON_SYSTEM,
                            "System",
                            false,
                            preview(t.fill(FillTier::Secondary)),
                        ))
                        .child(option_card(
                            &card_style,
                            ICON_LIGHT,
                            "Light",
                            true,
                            preview(gpui::white()),
                        ))
                        .child(option_card(
                            &card_style,
                            ICON_DARK,
                            "Dark",
                            false,
                            preview(gpui::black()),
                        )),
                ),
            )
            .into_any_element()
    })
}
