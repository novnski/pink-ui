//! Pink's surfaces: in-flow cards and settings rows, the popover / menu kit,
//! field frames, tooltips and dialogs (Pink `popover.rs`,
//! `settings/widgets.rs`, `surface_chrome.rs`, `transcript.rs`, `shell.rs`).
//!
//! Plates follow Pink's default Frosted treatment, each tint baked over the
//! canvas (`Pink::bg`) because GPUI here has no backdrop blur: a popover,
//! dialog or palette reads as the window behind it, lifted by its hairline,
//! its shadows and Pink's frost elevation rings (`Pink::elevation_rings`).
//! Tooltips stay opaque, as in Pink. In-flow cards are never shadowed.

use gpui::{
    AnyView, App, Context, Div, FontWeight, Hsla, IntoElement, Render, SharedString, Stateful,
    Window, div, prelude::*, px,
};

use super::{
    FillTier, Pink, SANS, TextStyle, TypeStyleExt, controls::glyphs, controls::line_glyph, layout,
    radius, shadow_lg,
};
use crate::HoverFill;
use crate::components::empty_state as empty;
use crate::components::menu_card as menu;
use crate::components::overlay_card as overlay;
use crate::components::settings_parts as parts;
use crate::components::settings_section as section;

// ---------------------------------------------------------------------------
// In-flow cards
// ---------------------------------------------------------------------------

/// A card resting on the panel: the settings section card's surface (radius
/// 12, 1 px `border`, `card_glass_bg()`: Pink's 40% frosted `surface` tint
/// over the canvas), without the section margin and clip, and with no shadow
/// (Pink never shadows in-flow content).
pub fn card(t: &Pink) -> Div {
    div()
        .bg(t.card_glass_bg())
        .border_1()
        .border_color(t.border)
        .rounded(px(radius::SECTION_CARD))
}

/// A settings section card (Pink `widgets::section_card`), drawn by the
/// `pink-settings-section` package: `mt 24`, radius 12, 1 px `border`,
/// `card_glass_bg()`, clipped, a column. Put a [`section_header`] inside,
/// then [`card_row`]s.
pub fn section_card(t: &Pink) -> Div {
    section::section_card(&section::CardStyle {
        border: t.border,
        background: t.card_glass_bg(),
    })
}

/// A section's title block inside its card (Pink `widgets::section_header`):
/// `px 16 / pt 14 / pb 12`, the title in Title3 (15/20 semibold) and an
/// optional description, gap 3. Trailing actions go in a [`row_controls`]
/// child. Follow it with `card_row(false, ..)` rows.
pub fn section_header(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    t: &Pink,
) -> Div {
    section::section_header().child(
        section::header_text()
            .child(section::header_title(t.text, title))
            .when_some(description, |column, copy| {
                column.child(row_description(copy, t))
            }),
    )
}

/// One settings card row (Pink `widgets::card_row`): at least 48 tall,
/// `px 16 / py 12`, a `border` hairline on top unless `first`, wrapping with
/// gaps 12 / 10. No hover: rows aren't controls.
pub fn card_row(first: bool, t: &Pink) -> Div {
    section::card_row((!first).then_some(t.border))
}

/// A row's text column (Pink `widgets::row_text`): title and optional
/// description, gap 2, holding 220 so the controls wrap below it before the
/// text squeezes.
pub fn row_text(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    t: &Pink,
) -> Div {
    section::row_text()
        .child(row_title(title, t))
        .when_some(description, |column, copy| {
            column.child(row_description(copy, t))
        })
}

/// A row title: 13/18 medium `text`, wrapping.
pub fn row_title(title: impl Into<SharedString>, t: &Pink) -> Div {
    section::row_title(t.text, title)
}

/// A row description: 12/16 in `readable_muted(0.85)`, wrapping.
pub fn row_description(copy: impl Into<SharedString>, t: &Pink) -> Div {
    section::row_description(t.readable_muted(0.85), copy)
}

/// A row's trailing controls (Pink `widgets::row_controls`): one line pushed
/// right with `ml_auto` (as in Pink, so it also stays right-aligned when the
/// row wraps it under the text), gap 8, never wrapping.
pub fn row_controls() -> Div {
    section::row_controls()
}

/// A full-width block under a row's text (chip groups, lists): wraps, gap 8.
pub fn row_block() -> Div {
    section::row_block()
}

/// A row's identity tile (Pink `widgets::row_tile`), drawn by the
/// `pink-settings-parts` package: 28 square, radius 7, 1 px `border`,
/// `fill(Quaternary)`, a 16 icon in `text_muted`.
pub fn row_tile(icon_path: impl Into<SharedString>, t: &Pink) -> Div {
    parts::row_tile(&parts::tile_style(t), icon_path)
}

/// Fine print under a section card: `mt 8 / px 16`, 12/16
/// `readable_muted(0.8)`.
pub fn section_footer(copy: impl Into<SharedString>, t: &Pink) -> Div {
    section::section_footer(t.readable_muted(0.8), copy)
}

/// A form field label above a group (Pink `widgets::field_label`): `mt 12 /
/// mb −4`, 13 semibold `readable_muted(0.85)`.
pub fn field_label(label: impl Into<SharedString>, t: &Pink) -> Div {
    div()
        .mt(px(12.0))
        .mb(px(-4.0))
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.readable_muted(0.85))
        .child(label.into())
}

/// A code and output surface (tool output, diffs): radius 10, 1 px `border`,
/// an `ink(0.035)` plate.
pub fn code_surface(t: &Pink) -> Div {
    div()
        .rounded(px(radius::PANEL))
        .border_1()
        .border_color(t.border)
        .bg(t.ink(0.035))
}

/// An inline card standing on its own (Pink's auth / empty-state card):
/// radius 12, `surface_card`, 1 px `border`, GPUI's `shadow_lg`.
pub fn inline_card(t: &Pink) -> Div {
    div()
        .rounded(px(radius::SECTION_CARD))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_card)
        .shadow(shadow_lg())
}

/// A hairline divider: 1 px of `separator(false)`.
pub fn divider(t: &Pink) -> Div {
    div().h(px(1.)).w_full().bg(t.separator(false))
}

/// The empty / loading / error recipe for a surface (Pink
/// `surface_chrome::surface_state`), drawn by the empty-state package's
/// surface state: a centred column taking the surface's free height, the
/// `glyph` (a 28 pt icon in `text_faint`, or a loader) over a 13 pt medium
/// title and an optional 12/18 pt `text_muted` body capped at 280 pt.
/// Chain an `empty::surface_action()` for its action.
pub fn surface_state(
    glyph: impl IntoElement,
    title: impl Into<SharedString>,
    body: Option<SharedString>,
    t: &Pink,
) -> Div {
    empty::surface()
        .child(empty::surface_glyph_slot().child(glyph))
        .child(empty::surface_title(t.text).child(title.into()))
        .when_some(body, |el, body| {
            el.child(empty::surface_body(t.text_muted).child(body))
        })
}

// ---------------------------------------------------------------------------
// Popovers and menus
// ---------------------------------------------------------------------------

/// The popover / menu card (Pink `popover::popover_card` in its frosted
/// mount): 1 px `border`, radius 12, the popover plate (`surface_bg()`, the
/// frosted tint over the canvas: #060606 / #fafcfe), Pink's frost elevation
/// rings over `menu_shadows()`, `p 4`, gap 2, clipped, 13 Geist in `text`.
/// Chain `.flex().flex_col()` for a menu, and a width from
/// `layout::MENU_W_*`. Text inside reads `t.for_popup()` tones.
pub fn popover_card(t: &Pink) -> Div {
    div()
        .border_1()
        .border_color(t.border)
        .rounded(px(radius::POPOVER))
        .shadow(floating_shadows(t.menu_shadows(), t))
        .bg(t.surface_bg())
        .p(px(layout::CARD_INSET))
        .gap(px(layout::MENU_GAP))
        .overflow_hidden()
        .text_size(px(13.0))
        .font_family(SANS)
        .text_color(t.text)
}

/// [`popover_card`] without its inset, for popovers with their own panes.
pub fn popover_card_flush(t: &Pink) -> Div {
    popover_card(t).p(px(0.0))
}

/// Kept for older call sites: the floating popover / menu surface is
/// [`popover_card`].
pub fn overlay(t: &Pink) -> Div {
    popover_card(t)
}

/// A full-width popup above the composer: the popover on the stack ladder's
/// popup step (radius 20, `p 6`), drawn by the `pink-overlay-card` package.
pub fn stack_popup_card(t: &Pink) -> Div {
    overlay::stack_popup(popover_card(t))
}

/// One pointer menu row (Pink `popover::menu_row`): gap 10, `px 8 / py 6`,
/// radius 8, 13. Active: `card_selected_bg()` and `text`. At rest the label
/// is `text @ 0.9` and the wash `wash(0)`, easing to `text` and
/// `menu_hover_bg()` (60% of the selected wash) under the pointer. A leading
/// icon is 16 px in `text_muted`.
pub fn menu_row(id: impl Into<SharedString>, active: bool, t: &Pink) -> Stateful<Div> {
    let id: SharedString = id.into();
    let row = div()
        .id(id.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(radius::MENU_ITEM))
        .text_size(px(13.0))
        .cursor_pointer();
    if active {
        row.bg(t.card_selected_bg()).text_color(t.text)
    } else {
        row.hover_fill_fg(
            format!("pink-menu-{id}"),
            t.wash(0.0),
            t.menu_hover_bg(),
            t.text.opacity(0.9),
            t.text,
        )
    }
}

/// A row of a keyboard-navigable menu (Pink `popover::menu_row_nav`): one
/// highlight that arrows and pointer share (`card_selected_bg()` on the
/// cursor row only, no separate hover wash), and a trailing 14 px check in
/// `text` on the current choice, in the 28 px check column. Move the cursor
/// from `.on_mouse_move`.
pub fn menu_row_nav(selected: bool, highlighted: bool, t: &Pink) -> Div {
    div()
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .pl(px(8.0))
        .pr(px(layout::MENU_CHECK_COLUMN))
        .py(px(6.0))
        .rounded(px(radius::MENU_ITEM))
        .text_size(px(13.0))
        .cursor_pointer()
        .text_color(if highlighted || selected {
            t.text
        } else {
            t.text.opacity(0.9)
        })
        .when(highlighted, |row| row.bg(t.card_selected_bg()))
        .when(selected, |row| {
            row.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px(8.0))
                    .flex()
                    .items_center()
                    .child(line_glyph(glyphs::CHECK, 14.0, t.text)),
            )
        })
}

/// [`menu_row_nav`] for a list with a cursor but no current choice (the
/// composer's completions): no check column, radius 14.
pub fn menu_row_cursor(highlighted: bool, t: &Pink) -> Div {
    menu_row_nav(false, highlighted, t)
        .pr(px(8.0))
        .rounded(px(radius::STACK_POPUP_ROW))
}

/// A submenu chevron for the end of a menu row: `alt-arrow-right` 14 in
/// `text_muted` (put a `flex_1` label before it).
pub fn menu_chevron(t: &Pink) -> gpui::Canvas<()> {
    line_glyph(glyphs::ALT_ARROW_RIGHT, 14.0, t.text_muted)
}

/// A small uppercase section heading inside a menu (Pink
/// `popover::menu_heading`), drawn by the `pink-menu-card` package: `px 8 /
/// pt 6 / pb 4`, 11 semibold in the popup's `text_muted` (`for_popup`: text
/// at 64%), tracked with hair spaces.
pub fn menu_heading(label: &str, t: &Pink) -> Div {
    menu::heading(t.for_popup().text_muted, label)
}

/// Uppercase with hair-space (U+200A) tracking, about 0.1em: GPUI has no
/// letter spacing (the `pink-menu-card` package's `tracked_upper`).
pub fn tracked_upper(label: &str) -> String {
    menu::tracked_upper(label)
}

/// The divider between menu sections (Pink `popover::menu_separator`), drawn
/// by the `pink-menu-card` package: 1 px of `separator(false)`, full-bleed
/// (`mx −4` cancels the card inset), `my 2`.
pub fn menu_separator(t: &Pink) -> Div {
    menu::separator(t.separator(false))
}

/// A bordered trailing menu block (Pink `popover::menu_section`), drawn by
/// the `pink-menu-card` package: `mt 4 / pt 4`, a `separator(false)` rule on
/// top, a column with gap 2.
pub fn menu_section(t: &Pink) -> Div {
    menu::section(t.separator(false))
}

/// A menu's empty line: `px 8 / py 24`, 12 `text_muted`, centered.
pub fn menu_empty(copy: impl Into<SharedString>, t: &Pink) -> Div {
    div()
        .px(px(8.0))
        .py(px(24.0))
        .text_center()
        .text_size(px(12.0))
        .text_color(t.text_muted)
        .child(copy.into())
}

// ---------------------------------------------------------------------------
// Field frames
// ---------------------------------------------------------------------------

/// The search field at the top of a picker popover (Pink
/// `popover::search_input_frame`), drawn by the `pink-menu-card` package:
/// `mb 4`, `px 10 / py 6`, radius 8, `fill(Quaternary)`, borderless, 13.
/// Add the `TextInput` as its child.
pub fn search_input_frame(t: &Pink) -> Div {
    menu::search_frame(t.fill(FillTier::Quaternary))
}

/// A dialog or form text field (Pink `popover::dialog_field`): full width,
/// `px 12 / py 8`, radius 8, a `separator(false)` edge, `fill(Quaternary)`,
/// 14. Pink frames don't change on focus: the caret (2 px, `t.caret`)
/// signals it.
pub fn dialog_field(t: &Pink) -> Div {
    div()
        .w_full()
        .px(px(12.0))
        .py(px(8.0))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(t.separator(false))
        .bg(t.fill(FillTier::Quaternary))
        .text_size(px(14.0))
}

/// A compact editor field (Pink automations `input_frame`): `px 10 / py 6`,
/// radius 8, a `separator(true)` edge, `fill(Quaternary)`, 13.
pub fn input_frame(t: &Pink) -> Div {
    div()
        .w_full()
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(t.separator(true))
        .bg(t.fill(FillTier::Quaternary))
        .text_size(px(13.0))
}

/// A toolbar field (Pink `surface_chrome::input`: file search, address
/// bar): 28 tall, `px 8`, radius 6, an `ink(0.035)` plate, gap 6, 12.
pub fn toolbar_input(t: &Pink) -> Div {
    div()
        .h(px(28.0))
        .min_w_0()
        .flex_1()
        .px(px(8.0))
        .rounded(px(radius::SM))
        .bg(t.ink(0.035))
        .flex()
        .items_center()
        .gap(px(6.0))
        .text_size(px(12.0))
}

// ---------------------------------------------------------------------------
// Tooltips
// ---------------------------------------------------------------------------

/// Every tooltip and hover card's surface (Pink `popover::tooltip_card`),
/// drawn by the `pink-overlay-card` package: radius 8, a 1 px
/// `border_strong` edge, the opaque `tooltip_surface()`, `tooltip_shadows()`,
/// and Geist named on the card (a tooltip lays out as its own root). Content
/// sets padding and width.
pub fn tooltip_card(t: &Pink) -> Div {
    overlay::tooltip_card(&overlay::tooltip_style(t))
}

/// The plain-text tooltip view (Pink `popover::TextTooltip`), drawn by the
/// `pink-overlay-card` package: the [`tooltip_card`] at `px 8 / py 5`,
/// Caption (11/14) in `text`, one line up to 48 characters, else wrapped at
/// 260.
pub struct TextTooltip(pub SharedString);

impl Render for TextTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        overlay::text_tooltip(&overlay::tooltip_style(&t), t.text, self.0.clone())
            .type_style(TextStyle::Caption)
    }
}

/// `.tooltip(pink::text_tooltip("Copy")).tooltip_show_delay(layout::TOOLTIP_DELAY)`.
pub fn text_tooltip(
    label: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label = label.into();
    move |_, cx| cx.new(|_| TextTooltip(label.clone())).into()
}

// ---------------------------------------------------------------------------
// Dialogs and palettes
// ---------------------------------------------------------------------------

/// A dialog card (Pink `popover::dialog_card` in its frosted `modal`), drawn
/// by the `pink-overlay-card` package: 360 wide, `p 20`, radius 16, the
/// popover plate, a `separator(false)` edge, the frost elevation rings over
/// `shadow_lg`, a column in `text`. Lay it on `t.scrim(MODAL_SCRIM)`.
pub fn dialog_card(t: &Pink) -> Div {
    overlay::dialog_card(&overlay::dialog_style(t))
        .shadow(floating_shadows(shadow_lg(), t))
        .font_family(SANS)
}

/// A dialog title: 15 semibold `text`. The body follows at `mt 6`.
pub fn dialog_title(title: impl Into<SharedString>, t: &Pink) -> Div {
    div()
        .text_size(px(15.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text)
        .child(title.into())
}

/// Dialog body copy: 13/19 `text_muted`. Controls follow at `mt 16`, the
/// button row `justify_end` with gap 8 and `ControlSize::Lg` buttons.
pub fn dialog_body(copy: impl Into<SharedString>, t: &Pink) -> Div {
    div()
        .text_size(px(13.0))
        .line_height(px(19.0))
        .text_color(t.text_muted)
        .child(copy.into())
}

/// A command-palette card (Pink `popover::palette_card`, mounted elevated),
/// drawn by the `pink-menu-card` package: `width`, radius `corner_radius`
/// (Pink uses 16), a `separator(false)` edge, the popover plate, clipped, a
/// column. The kit's plate is opaque, so the card lifts with the frost
/// elevation rings over `shadow_lg` rather than the package's plain one.
/// Rows inside are radius 8 ([`radius::PALETTE_ITEM`]); header and footer
/// rules are `hairline(0.06)`.
pub fn palette_card(width: f32, corner_radius: f32, t: &Pink) -> Div {
    menu::palette_card(
        &menu::PaletteStyle {
            border: t.separator(false),
            background: t.surface_bg(),
            text: t.text,
            font_family: SANS.into(),
        },
        px(width),
        corner_radius,
    )
    .shadow(floating_shadows(shadow_lg(), t))
}

/// A floating card's elevation: Pink's frost rings (a crisp contact rim and
/// an 8 px falloff) above the card's own drop shadow. The kit's plates are
/// opaque, so neither stains through.
fn floating_shadows(drop: Vec<gpui::BoxShadow>, t: &Pink) -> Vec<gpui::BoxShadow> {
    let mut shadows = t.elevation_rings();
    shadows.extend(drop);
    shadows
}

/// A dialog or palette scrim filling its parent: `t.scrim(MODAL_SCRIM)`
/// (black 35% dark, ≈19% light), centering its child.
pub fn modal_scrim(t: &Pink) -> Div {
    div()
        .size_full()
        .bg(t.scrim(super::MODAL_SCRIM))
        .flex()
        .items_center()
        .justify_center()
}

/// The tone a notice or strip paints in, for callers building one: the
/// plate, the edge and the copy of Pink's settings error / warning strips
/// (`tone @ 0.06`, `tone @ 0.2`, `readable_tone(muted, 0.9)`).
pub fn strip_colors(tone: Hsla, muted: Hsla, t: &Pink) -> (Hsla, Hsla, Hsla) {
    (
        tone.opacity(0.06),
        tone.opacity(0.2),
        t.readable_tone(muted, 0.9),
    )
}

#[cfg(test)]
mod tests {
    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*, px};

    use crate::{
        Theme, ThemeMode, motion,
        pink::{self, Pink},
    };

    /// The kit guide's example (a settings card and a menu), kept compiling
    /// and rendering in both appearances.
    struct GuideExample {
        on: bool,
    }

    impl Render for GuideExample {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let t = Pink::get(cx);
            let card = pink::section_card(&t)
                .child(pink::section_header(
                    "Notifications",
                    Some("How PINK reaches you.".into()),
                    &t,
                ))
                .child(
                    pink::card_row(false, &t)
                        .child(pink::row_text(
                            "Sounds",
                            Some("A chime when a turn ends.".into()),
                            &t,
                        ))
                        .child(pink::row_controls().child(
                            pink::switch_control("sounds", "Sounds", self.on, &t).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.on = !this.on;
                                    cx.notify()
                                }),
                            ),
                        )),
                );
            let menu = motion::menu_in(
                "new-menu",
                pink::popover_card(&t)
                    .flex()
                    .flex_col()
                    .w(px(pink::layout::MENU_W_S))
                    .child(pink::menu_heading("Create", &t))
                    .child(
                        pink::menu_row("new-chat", false, &t)
                            .child(pink::line_glyph(pink::glyphs::PLUS, 16., t.text_muted))
                            .child(div().flex_1().child("New chat"))
                            .child(pink::kbd_hint("⌘N", &t)),
                    )
                    .child(pink::menu_separator(&t)),
            );
            div().flex().flex_col().gap(px(16.)).child(card).child(menu)
        }
    }

    #[gpui::test]
    fn guide_example_renders_in_both_modes(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let (_, cx) = cx.add_window_view(|_, _| GuideExample { on: true });
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            cx.update(|window, cx| {
                Theme::set_mode(mode, cx);
                window.refresh();
            });
            cx.run_until_parked();
        }
    }
}
