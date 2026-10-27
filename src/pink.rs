//! Pink's design kit, for the Pink components.
//!
//! The core components keep their own tokens. The Pink components are drawn
//! the way the PINK desktop app draws itself, and this module is that app's
//! kit, value for value:
//!
//! - **Tokens** ([`Pink`]): the palette Pink paints at runtime for its default
//!   theme (`pink-dark` / `pink-light` with the default Violet accent), not
//!   the fallback literals in Pink's `Theme::dark()`/`Theme::light()`. At boot
//!   Pink resolves the registry variant (`crates/theme/src/builtins.rs`)
//!   through `Theme::from_variant` and then hardens every secondary tone
//!   against the surfaces it sits on (`harden_composited_foregrounds`,
//!   `crates/ui/src/theme.rs`). The kit reproduces Pink's default **Frosted**
//!   surface treatment, the one users see: every text tone is the one Pink
//!   hardens against its frosted reading surfaces (the glass over the
//!   adverse desktop, the floating, input and card tints over it).
//! - **Surfaces, baked over the canvas**: Pink's frosted plates are
//!   translucent tints over a 16 px backdrop blur. GPUI here has no backdrop
//!   blur, so a translucent tint would show the content behind it unblurred.
//!   The kit therefore takes each of Pink's frosted tints (`glass`,
//!   `glass_overlay`, `input_glass_bg`, `card_glass_bg` and the popover's
//!   `composer_sidebar_tint`) and flattens it over [`Pink::bg`], the canvas
//!   that sits behind every demo and stands in for the blurred backdrop.
//!   Every paint helper returns an opaque color: a popover is the near-canvas
//!   tint Pink's frosted card shows over a dark window, not the grey slab of
//!   the Opaque treatment. Floating cards keep their hairline and shadows,
//!   plus Pink's frost elevation rings, so they still separate from the
//!   canvas. Tooltips and toasts stay opaque, as in Pink.
//! - **Paint helpers** on [`Pink`]: Pink's `ink` / `hairline` / `wash` /
//!   `scrim` tiers, the text-tinted [`FillTier`] ladder and `separator`, the
//!   selection washes and rings, and the contrast floors (`readable_muted`,
//!   `readable_tone`, `readable_on`, `focus_ring`, `on_danger`), derived at
//!   paint time with Pink's own `legible_on` over Pink's reading surfaces.
//! - **Ladders**: [`radius`], [`space`], [`text`] / [`TextStyle`],
//!   [`icon_size`], the chrome control ladder ([`chrome`]) and the layout
//!   constants ([`layout`]), from Pink's `theme`, `typography`, `icons`,
//!   `surface_chrome` and `popover` modules.
//! - **Elevation**: `menu_shadows`, `tooltip_shadows`, GPUI's
//!   `shadow_sm/md/lg`, the 2 px inset focus ring and the 1 px selection ring,
//!   verbatim (GPUI `blur_radius` is a Gaussian sigma, as Pink writes it),
//!   and Pink's frosted-card elevation rings (`frost::paint_elevation`) as
//!   crisp box shadows ([`Pink::elevation_rings`]).
//! - **Control kit** (`controls`, `surfaces`, re-exported here): Pink's
//!   `button` module (buttons, icon buttons, selects, chips, segmented
//!   controls, selection rings), settings widgets (switch, section cards and
//!   rows, badges), popover kit (popover card, menu rows, headings,
//!   separators, kbd hints, key caps, field frames, tooltips, dialogs).
//!
//! Sizes are px at Pink's default interface size: Pink writes them as
//! `ui_rems(n)` = `rems(n / 16)` with the window rem at the 16 px default, so
//! `ui_rems(13.)` paints 13 px. Corners and hairlines are px in Pink too.
//!
//! ```ignore
//! let t = Pink::get(cx);
//! pink::section_card(&t)
//!     .child(pink::section_header("Notifications", None, &t))
//!     .child(pink::card_row(false, &t).child(pink::row_text("Sounds", None, &t)))
//! ```

use std::{
    cell::RefCell,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use gpui::{App, BoxShadow, FontFeatures, FontWeight, Hsla, Styled, hsla, px};

use crate::ActiveTheme;

pub mod controls;
pub mod loaders;
pub mod surfaces;

pub use controls::*;
pub use surfaces::*;

/// Interface size over its 16 px baseline (Pink `Theme::ui_scale`), from
/// the [`crate::Theme`] global. Geometry Pink scales with the Interface size
/// (hit sizes through [`chrome::scaled_control`], the titlebar cluster,
/// sidebar rows) multiplies by it; the rest of the kit is drawn at 16.
pub fn ui_scale(cx: &App) -> f32 {
    cx.theme().ui_scale()
}

/// Pink's interface font, `Theme::font_sans` (bundled under `assets/fonts`).
pub const SANS: &str = "Geist";
/// Pink's code and terminal font, `Theme::font_mono` / `font_terminal`.
pub const MONO: &str = "Geist Mono";
/// Pink's system fallback for the interface font (`theme.rs` `system_sans`).
pub const SANS_FALLBACK: &str = "Helvetica";
/// Pink's system fallback for the code font (`theme.rs` `system_mono`).
pub const MONO_FALLBACK: &str = "Menlo";

// ---------------------------------------------------------------------------
// Ladders
// ---------------------------------------------------------------------------

/// Corner radius ladder (Pink `theme::radius`, audit DS-10): pick by role, and
/// give a nested surface [`radius::inner`] of its container so corners stay
/// concentric. The named radii below are the ones Pink's builders use.
pub mod radius {
    // Roles (Pink `theme::radius`, UI rethink D37). New and migrated
    // surfaces pick one of these; the size names below stay for older
    // call sites, as in Pink.
    /// Controls and rows.
    pub const CONTROL: f32 = 6.0;
    /// Code blocks, notices and tooltips.
    pub const CODE: f32 = 8.0;
    /// Popovers, menus and the outer content frame (`popover::CARD_RADIUS`).
    pub const POPOVER: f32 = 12.0;
    /// Dialogs and sheets (`popover::dialog_card`).
    pub const DIALOG: f32 = 16.0;
    /// The composer plate role. Pink's composer itself still draws its pill
    /// at [`COMPOSER_PILL`] (`composer::COMPOSER_RADIUS`); this is the role
    /// new plates pick.
    pub const COMPOSER: f32 = 20.0;

    /// Inline code, kbd hints, checkboxes, file wells, progress tracks.
    pub const XS: f32 = 4.0;
    /// Controls up to 24 pt, chips, icon buttons, tiles, badges
    /// (`Theme::CONTROL_RADIUS`).
    pub const SM: f32 = 6.0;
    /// Controls 28–32 pt, menu rows, inputs.
    pub const MD: f32 = 8.0;
    /// Cards, code and output surfaces, notices (`Theme::PANEL_RADIUS`).
    pub const LG: f32 = 10.0;
    /// Dialogs, the command palette, message bubbles, sheets
    /// (`Theme::BUBBLE_RADIUS`).
    pub const XL: f32 = 16.0;

    /// `Theme::PANEL_RADIUS`: code surfaces, transcript notices, tool cards.
    pub const PANEL: f32 = LG;
    /// `Theme::BUBBLE_RADIUS`: the user message bubble.
    pub const BUBBLE: f32 = XL;
    /// Menu rows: concentric with the popover at its 4 pt inset
    /// (`popover::MENU_ITEM_RADIUS`).
    pub const MENU_ITEM: f32 = POPOVER - 4.0;
    /// Settings section cards (`settings::widgets::section_card`).
    pub const SECTION_CARD: f32 = 12.0;
    /// Tooltips and rich hover cards (`popover::tooltip_card`): the
    /// [`CODE`] role.
    pub const TOOLTIP: f32 = CODE;
    /// The command palette.
    pub const PALETTE: f32 = 16.0;
    /// Palette rows, concentric with the palette at its 8 pt inset
    /// (`popover::PALETTE_ITEM_RADIUS`).
    pub const PALETTE_ITEM: f32 = PALETTE - 8.0;
    /// The composer pill as Pink's composer draws it
    /// (`composer::COMPOSER_RADIUS`, 26; 22 docked). Composer-styled cards
    /// that sit where the composer does use it too.
    pub const COMPOSER_PILL: f32 = 26.0;
    /// Full-width popups stacked above the composer
    /// (`popover::STACK_POPUP_RADIUS`).
    pub const STACK_POPUP: f32 = 20.0;
    /// Rows inside a stack popup (`STACK_POPUP_ROW_RADIUS` = 20 − 6).
    pub const STACK_POPUP_ROW: f32 = 14.0;
    /// Trays above the composer (`STACK_TRAY_RADIUS`).
    pub const STACK_TRAY: f32 = 18.0;
    /// Notices above the composer (`STACK_NOTICE_RADIUS`).
    pub const STACK_NOTICE: f32 = 14.0;
    /// `kbd_hint` and `key_cap`.
    pub const KBD: f32 = 5.0;
    /// The settings row identity tile (`row_tile`).
    pub const TILE: f32 = 7.0;
    /// Error and warning strips (settings).
    pub const STRIP: f32 = 12.0;

    /// Radius of a surface inset `inset` pt inside one with radius `outer`.
    pub fn inner(outer: f32, inset: f32) -> f32 {
        (outer - inset).max(XS)
    }
}

/// Spacing on the 4 pt grid (Pink `theme::space`, audit DS-11). `S6` is the
/// icon-to-label gap, `S10` a control's horizontal padding.
pub mod space {
    pub const S2: f32 = 2.0;
    pub const S4: f32 = 4.0;
    pub const S6: f32 = 6.0;
    pub const S8: f32 = 8.0;
    pub const S10: f32 = 10.0;
    pub const S12: f32 = 12.0;
    pub const S16: f32 = 16.0;
    pub const S20: f32 = 20.0;
    pub const S24: f32 = 24.0;
    pub const S32: f32 = 32.0;

    /// `Theme::SPACE_XS`.
    pub const XS: f32 = 4.0;
    /// `Theme::SPACE_SM`: the sidebar gutter.
    pub const SM: f32 = 8.0;
    /// `Theme::SPACE_MD`.
    pub const MD: f32 = 12.0;
    /// `Theme::SPACE_LG`.
    pub const LG: f32 = 16.0;
    /// `Theme::TEXT_STACK_GAP`: between a title and the line under it.
    pub const TEXT_STACK_GAP: f32 = 1.0;

    // Roles (Pink `theme::space`, UI rethink D33): a 4 pt structure with
    // 2 pt optical adjustments, in pt at the 16 px default. Interface-scaled
    // layout multiplies them by [`super::ui_scale`].
    /// The only allowed off-grid nudge, for optical alignment.
    pub const OPTICAL: f32 = S2;
    /// Between an icon and its label.
    pub const ICON_LABEL: f32 = S6;
    /// Between sibling controls or rows in one group.
    pub const SIBLING: f32 = S8;
    /// Inside a control, from its edge to its content.
    pub const CONTROL_PAD: f32 = S10;
    /// From a panel's edge to its content.
    pub const PANEL_INSET: f32 = S16;
    /// Between sections of one surface.
    pub const SECTION: f32 = S24;
}

/// Type sizes (Pink `typography::TextStyle`, audit DS-01). Prefer
/// [`TextStyle`] with [`TypeStyleExt::type_style`], which sets the size, the
/// line height and the weight together.
pub mod text {
    /// 10/13: kbd hints, counts, uppercase badges only.
    pub const CAPTION2: f32 = 10.0;
    /// 11/14: chrome labels, meta, chips, sidebar secondary.
    pub const CAPTION: f32 = 11.0;
    /// 12/16: row descriptions, tool rows, menu secondary.
    pub const FOOTNOTE: f32 = 12.0;
    /// 13/18: default UI text, row titles, buttons, menu rows.
    pub const BODY: f32 = 13.0;
    /// 14/21: chat prose, composer input, dialog fields.
    pub const PROSE: f32 = 14.0;
    /// 15/20 semibold: card and dialog titles.
    pub const TITLE3: f32 = 15.0;
    /// 17/22 semibold: section titles.
    pub const TITLE2: f32 = 17.0;
    /// 20/25 semibold: page titles.
    pub const TITLE1: f32 = 20.0;
    /// 26/32 bold: empty states, hero.
    pub const DISPLAY: f32 = 26.0;

    /// Kept for older call sites: Pink's 14 pt step is `Prose`.
    pub const CALLOUT: f32 = PROSE;
    /// Kept for older call sites: Pink has no 16 pt step; card and dialog
    /// titles are `Title3` (15/20 semibold).
    pub const TITLE: f32 = TITLE3;
    /// Kept for older call sites: Pink's 20 pt step is `Title1`.
    pub const LARGE: f32 = TITLE1;

    /// Code blocks and diffs (`CODE_FONT_SIZE_DEFAULT`; absolute in Pink,
    /// never scaled with the interface size).
    pub const CODE: f32 = 12.5;
    /// The terminal (`TERMINAL_FONT_SIZE_DEFAULT`).
    pub const TERMINAL: f32 = 13.0;
}

/// The interface type scale (Pink `typography::TextStyle`): every step pairs a
/// size with its line height and default weight. Apply it with
/// [`TypeStyleExt::type_style`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    /// 10/13: kbd hints, counts, uppercase badges only.
    Caption2,
    /// 11/14: chrome labels, meta, chips, sidebar secondary.
    Caption,
    /// 12/16: row descriptions, tool rows, menu secondary.
    Footnote,
    /// 13/18: default UI text, row titles, buttons.
    Body,
    /// 14/21: chat prose, composer input.
    Prose,
    /// 15/20 semibold: card and dialog titles.
    Title3,
    /// 17/22 semibold: section titles.
    Title2,
    /// 20/25 semibold: page titles.
    Title1,
    /// 26/32 bold: empty states, hero.
    Display,
}

impl TextStyle {
    /// (size, line height) in px.
    pub const fn metrics(self) -> (f32, f32) {
        match self {
            Self::Caption2 => (10.0, 13.0),
            Self::Caption => (11.0, 14.0),
            Self::Footnote => (12.0, 16.0),
            Self::Body => (13.0, 18.0),
            Self::Prose => (14.0, 21.0),
            Self::Title3 => (15.0, 20.0),
            Self::Title2 => (17.0, 22.0),
            Self::Title1 => (20.0, 25.0),
            Self::Display => (26.0, 32.0),
        }
    }

    pub const fn size(self) -> f32 {
        self.metrics().0
    }

    pub const fn line_height(self) -> f32 {
        self.metrics().1
    }

    pub const fn weight(self) -> FontWeight {
        match self {
            Self::Title3 | Self::Title2 | Self::Title1 => FontWeight::SEMIBOLD,
            Self::Display => FontWeight::BOLD,
            _ => FontWeight::NORMAL,
        }
    }
}

/// What a piece of interface text *is*, mapped to size and line height in pt
/// at the 16 px default (Pink `typography::TypeRole`, UI rethink D34).
/// Hierarchy comes from the role's size together with its colour tier
/// ([`TEXT_CONTRAST_PRIMARY`] and below), never from size alone. New and
/// migrated surfaces pick a role; [`TextStyle`] remains for older callers,
/// as in Pink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeRole {
    /// 13/18: titlebar, toolbar, menu and row titles.
    Chrome,
    /// 14/20: tool activity sentences in the transcript.
    ToolActivity,
    /// 12/16: timestamps, counts and secondary row lines.
    Metadata,
    /// 11/14: keyboard shortcut hints only.
    KeyHint,
    /// 14/20: the composer's input.
    ComposerInput,
    /// 15/24: transcript prose.
    Prose,
}

impl TypeRole {
    /// (size, line height) in px at the 16 px default.
    pub const fn metrics(self) -> (f32, f32) {
        match self {
            Self::Chrome => (13.0, 18.0),
            Self::ToolActivity | Self::ComposerInput => (14.0, 20.0),
            Self::Metadata => (12.0, 16.0),
            Self::KeyHint => (11.0, 14.0),
            Self::Prose => (15.0, 24.0),
        }
    }

    pub const fn size(self) -> f32 {
        self.metrics().0
    }

    pub const fn line_height(self) -> f32 {
        self.metrics().1
    }
}

/// `.type_style(TextStyle::Body)`: size, line height and weight together;
/// `.type_role(TypeRole::Chrome)`: a role's size and line height
/// (Pink `typography::TypeStyleExt`).
pub trait TypeStyleExt: Styled + Sized {
    fn type_style(self, style: TextStyle) -> Self {
        self.text_size(px(style.size()))
            .line_height(px(style.line_height()))
            .font_weight(style.weight())
    }

    /// Apply a [`TypeRole`]'s size and line height. Weight stays with the
    /// caller: a role names the text's job, not its emphasis.
    fn type_role(self, role: TypeRole) -> Self {
        self.text_size(px(role.size()))
            .line_height(px(role.line_height()))
    }
}

impl<T: Styled> TypeStyleExt for T {}

/// Tabular figures for counters and timers, so ticking digits never shift
/// the text after them (Pink `typography::tabular_figures`):
/// `.font_features(pink::tabular_figures())`.
pub fn tabular_figures() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// Icon size ladder (Pink `icons::size`, audit DS-12), paired with the type
/// scale. Pink's glyphs are Solar Linear, 24×24 with 1.5 strokes.
pub mod icon_size {
    /// Filled status glyphs beside caption text only.
    pub const XS: f32 = 12.0;
    /// Toolbars, chips, menus (caption / footnote text).
    pub const SM: f32 = 14.0;
    /// Buttons, settings tiles, menu and list rows (body text).
    pub const MD: f32 = 16.0;
    /// Titlebar, empty states (title text and up).
    pub const LG: f32 = 20.0;
}

/// The chrome control ladder for toolbars, tab strips and sidebar rows
/// (Pink `surface_chrome`, audit SH-10). Push buttons use [`ControlSize`].
pub mod chrome {
    /// Inline row actions, tab close buttons, chips.
    pub const CONTROL_S: f32 = 20.0;
    /// Titlebar, toolbars, tab strips, sidebar header.
    pub const CONTROL_M: f32 = 28.0;
    /// Sidebar footer.
    pub const CONTROL_L: f32 = 32.0;
    pub const ICON_S: f32 = 12.0;
    pub const ICON_M: f32 = 16.0;
    pub const ICON_L: f32 = 18.0;
    pub const RADIUS_S: f32 = super::radius::XS;
    pub const RADIUS_M: f32 = super::radius::SM;
    pub const RADIUS_L: f32 = super::radius::MD;
    /// Between toolbar controls.
    pub const CONTROL_GAP: f32 = 4.0;
    /// A toolbar's side inset.
    pub const EDGE_INSET: f32 = 8.0;
    /// Where every surface's content starts.
    pub const CONTENT_INSET: f32 = 12.0;
    /// No clickable target is smaller than this, at any Interface size
    /// (`surface_chrome::MIN_HIT`, UI rethink D36). [`CONTROL_S`] is a
    /// visual size: give it this hit area.
    pub const MIN_HIT: f32 = 24.0;

    /// A control step scaled with the Interface size (`scale` is
    /// [`super::ui_scale`]), kept on the ladder: never below the minimum hit
    /// area, never above the large step (`surface_chrome::scaled_control`).
    pub fn scaled_control(step: f32, scale: f32) -> f32 {
        (step * scale).clamp(MIN_HIT, CONTROL_L)
    }

    /// A glyph scaled with its control, kept between the small and large
    /// glyphs (`surface_chrome::scaled_icon`).
    pub fn scaled_icon(step: f32, scale: f32) -> f32 {
        (step * scale).clamp(ICON_S, ICON_L)
    }

    /// How far a [`MIN_HIT`] box reaches past a control drawn at `visual`
    /// px on each side: shift an absolutely positioned control by it so its
    /// visual stays put. In-flow controls take the [`MIN_HIT`] size itself,
    /// as Pink's per-project New thread button does; negative margins in a
    /// flex row measure as zero width in GPUI.
    pub fn hit_outset(visual: f32) -> f32 {
        ((MIN_HIT - visual) / 2.0).max(0.0)
    }
}

/// Layout constants from Pink's `theme`, `popover`, `settings` and `shell`.
pub mod layout {
    use std::time::Duration;

    /// `Theme::TITLEBAR_HEIGHT`, also the toolbar row height.
    pub const TITLEBAR_HEIGHT: f32 = 38.0;
    pub const TITLEBAR_TOP_PAD: f32 = 4.0;
    /// Within-group rhythm for Back and Forward (`shell.rs`
    /// `TITLEBAR_CONTROL_GAP`).
    pub const TITLEBAR_CONTROL_GAP: f32 = 2.0;
    /// Between titlebar groups: the sidebar toggle, then navigation
    /// (`TITLEBAR_GROUP_GAP`, `Theme::SPACE_SM`).
    pub const TITLEBAR_GROUP_GAP: f32 = 8.0;
    /// From the control cluster to the title (`TITLEBAR_IDENTITY_GAP`,
    /// `Theme::SPACE_MD`): the 12 pt kept before the title at every size.
    pub const TITLEBAR_IDENTITY_GAP: f32 = 12.0;
    /// The control row's own horizontal inset (`TITLEBAR_CLUSTER_PAD`).
    pub const TITLEBAR_CLUSTER_PAD: f32 = 10.0;

    /// Each top-left window control at Interface `scale` (`shell.rs`
    /// `titlebar_control_size`): the medium step grown with the text beside
    /// it, between the 24 pt minimum hit and the 32 pt step that still fits
    /// the 38 pt band.
    pub fn titlebar_control_size(scale: f32) -> f32 {
        super::chrome::scaled_control(super::chrome::CONTROL_M, scale)
    }

    /// The width the sidebar toggle, Back and Forward draw at `scale`
    /// (`shell.rs` `cluster_buttons_width`), computed from the same size the
    /// buttons render with so the title's reservation can't drift from them.
    pub fn cluster_buttons_width(scale: f32) -> f32 {
        3.0 * titlebar_control_size(scale) + TITLEBAR_GROUP_GAP + TITLEBAR_CONTROL_GAP
    }
    pub const STATUS_STRIP_HEIGHT: f32 = 24.0;
    /// The fade band at a scrolling edge (`TRANSCRIPT_FADE_BAND`).
    pub const FADE_BAND: f32 = 24.0;

    /// The popover card's inset (`popover::CARD_INSET`).
    pub const CARD_INSET: f32 = 4.0;
    /// Between menu rows (`popover::MENU_GAP`).
    pub const MENU_GAP: f32 = 2.0;
    /// Menu widths (`MENU_W_S/M/L`): content may widen a card, never shrink it.
    pub const MENU_W_S: f32 = 240.0;
    pub const MENU_W_M: f32 = 280.0;
    pub const MENU_W_L: f32 = 320.0;
    /// Right padding a keyboard menu row reserves for its check.
    pub const MENU_CHECK_COLUMN: f32 = 28.0;
    /// Trigger → popover gap (`pt(6)` below, `pb(6)` above).
    pub const POPOVER_GAP: f32 = 6.0;
    /// Popovers keep this far from the window edge.
    pub const WINDOW_MARGIN: f32 = 8.0;
    /// The stack popup's inset above the composer.
    pub const STACK_POPUP_INSET: f32 = 6.0;

    /// Widest a tooltip grows before its text wraps.
    pub const TOOLTIP_MAX_WIDTH: f32 = 260.0;
    /// Labels longer than this wrap at [`TOOLTIP_MAX_WIDTH`].
    pub const TOOLTIP_WRAP_CHARS: usize = 48;
    /// Shared tooltip show delay (`popover::TOOLTIP_DELAY`, audit DS-20).
    pub const TOOLTIP_DELAY: Duration = Duration::from_millis(400);
    /// A tooltip shown above its trigger sits this far above it.
    pub const TOOLTIP_ABOVE_GAP: f32 = 7.0;

    /// `popover::dialog_card`.
    pub const DIALOG_WIDTH: f32 = 360.0;
    pub const DIALOG_PAD: f32 = 20.0;

    /// Settings: page column, section gap, row anatomy.
    pub const PAGE_MAX_WIDTH: f32 = 720.0;
    pub const SECTION_GAP: f32 = 24.0;
    pub const ROW_PAD_X: f32 = 16.0;
    pub const ROW_PAD_Y: f32 = 12.0;
    pub const ROW_MIN_HEIGHT: f32 = 48.0;
    pub const ROW_TEXT_MIN: f32 = 220.0;

    /// Sidebar widths.
    pub const SIDEBAR_MIN: f32 = 224.0;
    pub const SIDEBAR_DEFAULT: f32 = 256.0;
    pub const SIDEBAR_MAX: f32 = 400.0;
    /// The composer column.
    pub const COMPOSER_MAX_WIDTH: f32 = 768.0;
    /// Composer-row controls: 28 pt capsules and circles.
    pub const COMPOSER_CONTROL: f32 = 28.0;

    /// Caret blink half-period (`composer::CARET_BLINK_MS`), caret width 2.
    pub const CARET_BLINK_MS: u64 = 500;
    pub const CARET_WIDTH: f32 = 2.0;
}

// ---------------------------------------------------------------------------
// Color math (Pink `theme.rs`, verbatim)
// ---------------------------------------------------------------------------

/// WCAG 2.2 AA minimum for normal-size text.
pub const TEXT_CONTRAST_MIN: f32 = 4.5;
/// WCAG 2.2 AA minimum for meaningful non-text: icons, focus visuals.
pub const NON_TEXT_CONTRAST_MIN: f32 = 3.0;
/// What derived text colors aim for (Pink `TEXT_CONTRAST_TARGET`).
pub const TEXT_CONTRAST_TARGET: f32 = 4.6;
/// What derived glyph and ring colors aim for (`NON_TEXT_CONTRAST_TARGET`).
pub const NON_TEXT_CONTRAST_TARGET: f32 = 3.1;
/// Text contrast tiers on the reading surfaces (Pink `theme.rs`, UI rethink
/// D38): primary about 12:1, secondary about 6:1, tertiary on the target.
/// Nothing that is text, metadata and placeholders included, goes below
/// [`TEXT_CONTRAST_MIN`].
pub const TEXT_CONTRAST_PRIMARY: f32 = 12.0;
pub const TEXT_CONTRAST_SECONDARY: f32 = 6.0;
pub const TEXT_CONTRAST_TERTIARY: f32 = TEXT_CONTRAST_TARGET;
/// Light hairlines are 1.35× stronger than dark ones, capped at 0.5
/// (`INK_HAIRLINE_SCALE`). Fills keep the same alpha (`INK_FILL_SCALE` 1.0).
pub const INK_HAIRLINE_SCALE: f32 = 1.35;
/// The standard modal backdrop's dark-mode alpha (`SCRIM_ALPHA_DARK`).
pub const SCRIM_ALPHA_DARK: f32 = 0.60;
/// Dark frost coverage of the shell glass over the desktop
/// (`Theme::GLASS_ALPHA`), raised if shell text would not read.
pub const GLASS_ALPHA: f32 = 0.80;
/// Light frost coverage (`Theme::GLASS_ALPHA_LIGHT`): heavier, since dark
/// text is more vulnerable to the desktop showing through.
pub const GLASS_ALPHA_LIGHT: f32 = 0.94;
/// The scrim behind dialogs and palettes (`popover::MODAL_SCRIM`).
pub const MODAL_SCRIM: f32 = 0.35;

/// OKLCH → color (CSS Color 4 matrices), as Pink writes its oklch tokens.
pub fn oklch(l: f32, c: f32, h_deg: f32) -> Hsla {
    let h = h_deg.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_93 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_4 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    let enc = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    gpui::Rgba {
        r: enc(r),
        g: enc(g),
        b: enc(b),
        a: 1.0,
    }
    .into()
}

/// An achromatic tone at OKLCH lightness `l` (Tailwind's neutral scale).
pub fn neutral(l: f32) -> Hsla {
    oklch(l, 0.0, 0.0)
}

/// An exact achromatic tone from an 8-bit channel (`grey(13)` = `#0d0d0d`).
pub fn grey(value: u8) -> Hsla {
    hsla(0.0, 0.0, value as f32 / 255.0, 1.0)
}

/// `#rrggbb` as a color.
fn hex(rgb: u32) -> Hsla {
    gpui::rgb(rgb).into()
}

/// `#rrggbbaa`: Pink's palette stores 8-bit alphas (`Color::with_alpha`).
fn hexa(rgb: u32, alpha: u8) -> Hsla {
    Hsla {
        a: alpha as f32 / 255.0,
        ..hex(rgb)
    }
}

/// HSL (GPUI convention, all 0..1) → sRGB components 0..1.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s <= f32::EPSILON {
        return [l, l, l];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0)]
}

/// sRGB (0..1) → HSL (GPUI convention).
fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let delta = max - min;
    if delta < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    } / 6.0;
    (h, s, l)
}

/// WCAG relative luminance of an opaque color.
pub fn relative_luminance(color: Hsla) -> f32 {
    let lin = |c: f32| {
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = hsl_to_rgb(color.h, color.s, color.l);
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// WCAG contrast ratio between two opaque colors (1 … 21).
pub fn contrast_ratio(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Contrast of a possibly translucent `fg` as painted on opaque `bg`.
pub fn painted_contrast(fg: Hsla, bg: Hsla) -> f32 {
    contrast_ratio(flatten(fg, bg), bg)
}

/// Composite `fg` (maybe translucent) over an opaque `bg`: the color the eye
/// receives (Pink `theme::flatten`).
pub fn flatten(fg: Hsla, bg: Hsla) -> Hsla {
    let a = fg.a.clamp(0.0, 1.0);
    let [fr, fg_, fb] = hsl_to_rgb(fg.h, fg.s, fg.l);
    let [br, bg_, bb] = hsl_to_rgb(bg.h, bg.s, bg.l);
    let (h, s, l) = rgb_to_hsl(
        fr * a + br * (1.0 - a),
        fg_ * a + bg_ * (1.0 - a),
        fb * a + bb * (1.0 - a),
    );
    hsla(h, s, l, 1.0)
}

/// A label for solid `fills`: `authored` when it reaches the text target on
/// all of them, otherwise white or black, whichever reads better (Pink
/// `theme::on_fill`).
pub fn on_fill(authored: Hsla, fills: &[Hsla]) -> Hsla {
    let worst = |candidate: Hsla| {
        fills
            .iter()
            .map(|fill| painted_contrast(candidate, *fill))
            .fold(f32::MAX, f32::min)
    };
    if worst(authored) >= TEXT_CONTRAST_TARGET {
        return authored;
    }
    let (white, black) = (hsla(0.0, 0.0, 1.0, 1.0), hsla(0.0, 0.0, 0.0, 1.0));
    if worst(white) >= worst(black) {
        white
    } else {
        black
    }
}

/// `color` as asked when it clears `min` on every `surfaces` entry; else its
/// opacity rises, then its lightness moves (toward the side `text` reads on,
/// then the other) until it does; hue and saturation are kept (Pink
/// `theme::legible_on`).
pub fn legible_on(color: Hsla, text: Hsla, surfaces: &[Hsla], min: f32) -> Hsla {
    let worst = |candidate: Hsla| {
        surfaces
            .iter()
            .map(|surface| painted_contrast(candidate, *surface))
            .fold(f32::MAX, f32::min)
    };
    if worst(color) >= min {
        return color;
    }
    for step in 1..=10 {
        let alpha = color.a + (1.0 - color.a) * step as f32 / 10.0;
        let raised = Hsla { a: alpha, ..color };
        if worst(raised) >= min {
            return raised;
        }
    }
    let opaque = Hsla { a: 1.0, ..color };
    let lighter_first = relative_luminance(text) >= relative_luminance(opaque);
    let targets = if lighter_first {
        [1.0, 0.0]
    } else {
        [0.0, 1.0]
    };
    for target in targets {
        for step in 1..=24 {
            let l = opaque.l + (target - opaque.l) * step as f32 / 24.0;
            let shifted = Hsla { l, ..opaque };
            if worst(shifted) >= min {
                return shifted;
            }
        }
    }
    let (white, black) = (hsla(0.0, 0.0, 1.0, 1.0), hsla(0.0, 0.0, 0.0, 1.0));
    if worst(white) >= worst(black) {
        white
    } else {
        black
    }
}

/// A shadow with GPUI's own blur semantics (a Gaussian sigma), exactly as
/// Pink writes its elevations. `primitives::shadow` converts CSS values
/// instead, which would halve these.
fn shadow(x: f32, y: f32, sigma: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: gpui::point(px(x), px(y)),
        blur_radius: px(sigma),
        spread_radius: px(spread),
        inset: false,
    }
}

/// An inset ring of `width` px: painted on top of the fill, edges only, with
/// no layout cost.
fn inset_ring(width: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: gpui::point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(width),
        inset: true,
    }
}

/// GPUI's `shadow_sm()` as a list (Pink's switch knob, small pills).
pub fn shadow_sm() -> Vec<BoxShadow> {
    vec![
        shadow(0., 1., 3., 0., hsla(0., 0., 0., 0.1)),
        shadow(0., 1., 2., -1., hsla(0., 0., 0., 0.1)),
    ]
}

/// GPUI's `shadow_md()` as a list (floating chrome pieces).
pub fn shadow_md() -> Vec<BoxShadow> {
    vec![
        shadow(0., 4., 6., -1., hsla(0., 0., 0., 0.1)),
        shadow(0., 2., 4., -2., hsla(0., 0., 0., 0.1)),
    ]
}

/// GPUI's `shadow_lg()` as a list (dialogs, palettes, the composer and
/// inline cards).
pub fn shadow_lg() -> Vec<BoxShadow> {
    vec![
        shadow(0., 10., 15., -3., hsla(0., 0., 0., 0.1)),
        shadow(0., 4., 6., -4., hsla(0., 0., 0., 0.1)),
    ]
}

/// The keyboard focus ring for `.focus_visible(..)`: a 2 px inset shadow in
/// `color`, above the fill and below the children, adding nothing to layout
/// (Pink `theme::focus_ring`):
/// `.focus_visible(move |s| pink::focus_ring(s, ring))` with
/// `let ring = t.focus_ring();`.
pub fn focus_ring(style: gpui::StyleRefinement, color: Hsla) -> gpui::StyleRefinement {
    style.shadow(vec![inset_ring(2.0, color)])
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// Pink's interaction fill ladder (`theme::FillTier`, audit DS-05): one set of
/// weights for every hover, press and selection wash, tinted by the palette's
/// text hue (see [`Pink::fill`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillTier {
    /// Resting plates (ghost and secondary buttons, wells): dark 4%, light 3%.
    Quaternary,
    /// Hover: dark 8%, light 5%.
    Tertiary,
    /// Pressed, or hover on an already-filled control: dark 12%, light 8%.
    Secondary,
    /// Selected: dark 16%, light 10%.
    Primary,
}

impl FillTier {
    /// The tier's alpha in this appearance.
    pub const fn alpha(self, dark: bool) -> f32 {
        match (self, dark) {
            (FillTier::Quaternary, true) => 0.04,
            (FillTier::Tertiary, true) => 0.08,
            (FillTier::Secondary, true) => 0.12,
            (FillTier::Primary, true) => 0.16,
            (FillTier::Quaternary, false) => 0.03,
            (FillTier::Tertiary, false) => 0.05,
            (FillTier::Secondary, false) => 0.08,
            (FillTier::Primary, false) => 0.10,
        }
    }
}

/// Pink's resolved syntax colors (`Theme::syntax`), hardened over the
/// frosted reading surfaces and the diff washes. Keys that share a color
/// share a field.
#[derive(Clone, Copy, Debug)]
pub struct Syntax {
    pub comment: Hsla,
    /// keyword, macro, operator, variableSpecial.
    pub keyword: Hsla,
    pub string: Hsla,
    /// stringSpecial, escape, attribute.
    pub string_special: Hsla,
    /// number, boolean, constant.
    pub number: Hsla,
    /// type, typeBuiltin, constructor.
    pub type_name: Hsla,
    /// function, functionBuiltin, label.
    pub function: Hsla,
    /// property, tag.
    pub property: Hsla,
    /// variable, parameter.
    pub variable: Hsla,
    pub punctuation: Hsla,
    /// markupHeading, markupStrong.
    pub markup_heading: Hsla,
    pub markup_raw: Hsla,
    /// markupLink, markupEmphasis.
    pub markup_link: Hsla,
    pub markup_reference: Hsla,
    pub invalid: Hsla,
}

/// Pink's tokens for the active light/dark mode: the runtime palette of
/// `pink-dark` / `pink-light` with the Violet accent, Frosted treatment (the
/// app default), hardened as Pink hardens it.
///
/// Read them with [`Pink::get`]. Colors Pink derives at paint time are
/// methods: [`fill`](Self::fill), [`separator`](Self::separator),
/// [`focus_ring`](Self::focus_ring), the `readable_*` floors and the
/// surface recipes (opaque, baked over [`bg`](Self::bg)).
#[derive(Clone, Debug)]
pub struct Pink {
    pub dark: bool,
    /// The content panel and window (`background` seed): #060606 / #fbfcfe.
    pub bg: Hsla,
    /// Shell and sidebar tone (`shell` seed): #0d0d0d / #f0f3f7. Pink tints
    /// its glass and settings cards with it; paint
    /// [`chrome_surface`](Self::chrome_surface) or
    /// [`card_glass_bg`](Self::card_glass_bg).
    pub surface: Hsla,
    /// Opaque raised pills and discs (`raised` seed): #343438 / #e7ecf2.
    pub surface_raised: Hsla,
    /// Pink resolves this to `raised` as well.
    pub surface_raised_hover: Hsla,
    /// Inline cards resting on the panel (`card` seed): #0e0e0e / #ffffff.
    pub surface_card: Hsla,
    /// Dialog tone (`card` mixed toward `raised`): #151516 / #fefefe.
    pub surface_dialog: Hsla,
    /// The floating overlay tone (#1b1b1c / #ffffff) Pink tints floating
    /// cards with; paint [`glass_overlay`](Self::glass_overlay) or
    /// [`tooltip_surface`](Self::tooltip_surface).
    pub surface_overlay: Hsla,
    /// Chrome hover wash: white 11% / black 6%.
    pub element_hover: Hsla,
    /// Chrome selected wash: accent 18% / 10%.
    pub element_active: Hsla,
    /// Hairline border: white 10% / black 12%.
    pub border: Hsla,
    /// Strong hairline (tooltips, capsule actions), hardened to 3.1:1 on the
    /// frosted reading surfaces: white 51% / black 45%.
    pub border_strong: Hsla,
    pub text: Hsla,
    /// Secondary chrome text, hardened to 4.6:1 on every frosted reading
    /// surface: #c9c9cc / #445064.
    pub text_muted: Hsla,
    /// Placeholders and hints in chrome, hardened: #ccccce / #4d586a. On dark
    /// it lands a shade above `text_muted`, as in Pink.
    pub text_faint: Hsla,
    /// Pink's `text_dim`, hardened (resolves to `text_muted`).
    pub text_dim: Hsla,
    /// Secondary text on the content canvas (transcript): #a9a9ae / #445064.
    pub content_muted: Hsla,
    /// Tertiary text on the canvas: timestamps, timers (#8f8f94 / #586579).
    pub content_faint: Hsla,
    /// The composer's placeholder, floored on the composer plates
    /// (#949499 / #586579).
    pub placeholder: Hsla,
    /// Solid plate (Send disc, accents of solid): #ebebef / #232328.
    pub solid: Hsla,
    pub on_solid: Hsla,
    pub accent: Hsla,
    /// Accent for solid fills (switch on-track): equals `accent` for Violet.
    pub accent_strong: Hsla,
    /// Selected chip plate: accent 22% / 12%.
    pub accent_wash: Hsla,
    /// Label on an accent plate: black on dark, white on light.
    pub on_accent: Hsla,
    pub danger: Hsla,
    /// Danger copy (error text), hardened.
    pub danger_muted: Hsla,
    /// The Danger button plate (resolves to `danger`).
    pub danger_strong: Hsla,
    pub warning: Hsla,
    pub warning_muted: Hsla,
    pub success: Hsla,
    pub success_muted: Hsla,
    /// The working indicator's tone (`activity`): the accent.
    pub busy: Hsla,
    /// The accent glyph palette `[light, mid, deep]`, in Pink's
    /// `GlyphPalette::rows` order (loaders, the mini glyph spinner).
    pub glyph: [Hsla; 3],
    /// The input tint (`raised` at 72% on dark). Paint
    /// [`input_glass_bg`](Self::input_glass_bg) for the input plate.
    pub input_bg: Hsla,
    /// Text selection wash: accent 35% / 24%.
    pub selection: Hsla,
    /// The text caret (2 px, full line height): the accent.
    pub caret: Hsla,
    /// The terminal block cursor.
    pub cursor: Hsla,
    /// Inline code text, hardened on its wash.
    pub code_text: Hsla,
    /// Inline code plate: accent 22% / 12%.
    pub code_wash: Hsla,
    pub diff_add: Hsla,
    pub diff_del: Hsla,
    /// Hunk header band: accent 8% / 7%.
    pub diff_hunk_bg: Hsla,
    pub terminal_bg: Hsla,
    pub terminal_fg: Hsla,
    pub terminal_selection: Hsla,
    /// `separator(false)`: text-tinted 8% (light 8% × 1.35).
    pub separator: Hsla,
    /// `separator(true)`: text-tinted 14% (light 14% × 1.35).
    pub separator_strong: Hsla,
}

thread_local! {
    /// The appearance of the last [`Pink::get`], for context-free builders
    /// such as [`glyph`] (Pink's `CURRENT_GLYPH_TONE` and friends).
    static CURRENT_DARK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Paint-time memo for derived foregrounds (Pink `Theme::memo_color`).
    static MEMO: RefCell<HashMap<(u64, Vec<u32>), Hsla>> = RefCell::new(HashMap::new());
    /// Pink's frosted tints per palette (Pink memoizes the glass search too).
    static FROST: RefCell<HashMap<u64, Frost>> = RefCell::new(HashMap::new());
}

/// Pink's frosted tints for one palette, translucent, before baking.
#[derive(Clone, Copy, Debug)]
struct Frost {
    /// The shell glass (`Theme::glass`).
    glass: Hsla,
    /// Floating cards (`Theme::glass_overlay`).
    overlay: Hsla,
    /// Input plates (`Theme::input_glass_bg`).
    input: Hsla,
    /// Settings cards (`Theme::card_glass_bg`).
    card: Hsla,
    /// Popovers, dialogs and the composer (`Theme::composer_sidebar_tint`).
    composer: Hsla,
}

impl Pink {
    /// The tokens for the active mode.
    pub fn get(cx: &App) -> Self {
        let dark = cx.is_dark();
        CURRENT_DARK.with(|current| current.set(dark));
        Self::for_mode(dark)
    }

    /// The tokens for `dark` or light.
    pub fn for_mode(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }

    /// The tokens of the most recent [`Pink::get`] (light before any), for
    /// builders that take no `&Pink`.
    pub fn current() -> Self {
        Self::for_mode(CURRENT_DARK.with(|current| current.get()))
    }

    /// `pink-dark`, Violet accent, Frosted treatment.
    pub fn dark() -> Self {
        let text = hex(0xe8e8ea);
        let accent = hex(0x8b7cf6);
        let tint = fill_tint(text);
        Pink {
            dark: true,
            bg: hex(0x060606),
            surface: hex(0x0d0d0d),
            surface_raised: hex(0x343438),
            surface_raised_hover: hex(0x343438),
            surface_card: hex(0x0e0e0e),
            surface_dialog: hex(0x151516),
            surface_overlay: hex(0x1b1b1c),
            element_hover: hexa(0xffffff, 0x1c),
            element_active: hexa(0x8b7cf6, 0x2e),
            border: hexa(0xffffff, 0x1a),
            // hairline(0.18) hardened to 3.1:1 over the frosted reading
            // surfaces.
            border_strong: hsla(0.0, 0.0, 1.0, 0.508_235),
            text,
            // #a9a9ae hardened to 4.6:1: the glass over the white desktop
            // reads #3d3d3d, so muted lightens (#c9c9cc).
            text_muted: hsla(0.666_667, 0.029_940, 0.795_343, 1.0),
            // #85858a hardened the same way, a shade past muted (#ccccce).
            text_faint: hsla(0.666_667, 0.020_921, 0.804_739, 1.0),
            text_dim: hsla(0.666_667, 0.029_940, 0.795_343, 1.0),
            content_muted: hex(0xa9a9ae),
            // #85858a floored on the canvas and its code plate: two
            // lightness steps (#8f8f94).
            content_faint: hsla(0.666_667, 0.020_921, 0.570_425, 1.0),
            // #85858a floored on the composer plates (#949499).
            placeholder: hsla(0.666_667, 0.020_921, 0.589_951, 1.0),
            solid: hex(0xebebef),
            on_solid: hex(0x000000),
            accent,
            accent_strong: accent,
            accent_wash: hexa(0x8b7cf6, 0x38),
            on_accent: hex(0x000000),
            danger: hex(0xf87171),
            // #f49293 hardened (#f8bbbc).
            danger_muted: hsla(0.998_299, 0.816_667, 0.852_941, 1.0),
            danger_strong: hex(0xf87171),
            warning: hex(0xfacc15),
            warning_muted: hex(0xf6d34a),
            success: hex(0x34d399),
            // #61d8ad hardened (#75ddb7).
            success_muted: hsla(0.439_776, 0.604_061, 0.662_010, 1.0),
            busy: accent,
            glyph: [hex(0xaba1f9), accent, hex(0x7266ca)],
            input_bg: hexa(0x343438, 0xb8),
            selection: hexa(0x8b7cf6, 0x59),
            caret: accent,
            cursor: hexa(0xe8e8ea, 0x66),
            // The accent hardened on its own wash over the frosted canvas
            // (#ada2f9).
            code_text: hsla(0.687_159, 0.871_429, 0.805_556, 1.0),
            code_wash: hexa(0x8b7cf6, 0x38),
            diff_add: hex(0x34d399),
            diff_del: hex(0xf87171),
            diff_hunk_bg: hexa(0x8b7cf6, 0x14),
            terminal_bg: hex(0x090909),
            terminal_fg: text,
            terminal_selection: hexa(0xffffff, 0x38),
            separator: separator_for(true, tint, false),
            separator_strong: separator_for(true, tint, true),
        }
    }

    /// `pink-light`, Violet accent, Frosted treatment.
    pub fn light() -> Self {
        let text = hex(0x202631);
        let accent = hex(0x5b43e8);
        let tint = fill_tint(text);
        Pink {
            dark: false,
            bg: hex(0xfbfcfe),
            surface: hex(0xf0f3f7),
            surface_raised: hex(0xe7ecf2),
            surface_raised_hover: hex(0xe7ecf2),
            surface_card: hex(0xffffff),
            surface_dialog: hex(0xfefefe),
            surface_overlay: hex(0xffffff),
            element_hover: hexa(0x000000, 0x0f),
            element_active: hexa(0x5b43e8, 0x1a),
            border: hexa(0x000000, 0x1f),
            // black 22% hardened to 3.1:1 over the frosted reading surfaces.
            border_strong: hsla(0.0, 0.0, 0.0, 0.453_726),
            text,
            text_muted: hex(0x445064),
            // #586579 hardened to 4.6:1 over the glass on the black desktop
            // (#e2e4e8): three lightness steps toward black (#4d586a).
            text_faint: hsla(0.601_010, 0.157_895, 0.358_578, 1.0),
            text_dim: hex(0x445064),
            content_muted: hex(0x445064),
            content_faint: hex(0x586579),
            placeholder: hex(0x586579),
            solid: hex(0x232328),
            on_solid: hex(0xffffff),
            accent,
            accent_strong: accent,
            accent_wash: hexa(0x5b43e8, 0x1f),
            on_accent: hex(0xffffff),
            danger: hex(0xdc2626),
            danger_muted: hex(0xa72629),
            danger_strong: hex(0xdc2626),
            warning: hex(0xa16207),
            // #815312 hardened (#7c5011).
            warning_muted: hsla(0.097_598, 0.755_102, 0.276_226, 1.0),
            success: hex(0x15803d),
            // #186a3a hardened (#176638).
            success_muted: hsla(0.402_439, 0.630_769, 0.244_281, 1.0),
            busy: accent,
            glyph: [hex(0x7864ec), accent, hex(0x4332ac)],
            input_bg: hex(0xffffff),
            selection: hexa(0x5b43e8, 0x3d),
            caret: accent,
            cursor: hexa(0x202631, 0x8c),
            code_text: accent,
            code_wash: hexa(0x5b43e8, 0x1f),
            diff_add: hex(0x15803d),
            diff_del: hex(0xdc2626),
            diff_hunk_bg: hexa(0x5b43e8, 0x12),
            terminal_bg: hex(0xfafafa),
            terminal_fg: text,
            terminal_selection: hexa(0x000000, 0x29),
            separator: separator_for(false, tint, false),
            separator_strong: separator_for(false, tint, true),
        }
    }

    /// Pink's resolved syntax palette for this appearance.
    pub fn syntax(&self) -> Syntax {
        if self.dark {
            Syntax {
                comment: hex(0xc9c9cd),
                keyword: hex(0xcac3fb),
                string: hex(0x67deb2),
                string_special: hex(0x47daf1),
                number: hex(0xfacc15),
                type_name: hex(0xddbcfd),
                function: hex(0xa9cefc),
                property: hex(0xfab9db),
                variable: hex(0xe8e8ea),
                punctuation: hex(0xc8c8cd),
                markup_heading: hex(0xc2c6f5),
                markup_raw: hex(0x4ae1b2),
                markup_link: hex(0xf3bcda),
                markup_reference: hex(0xe7c66d),
                invalid: hex(0xfcb8b8),
            }
        } else {
            Syntax {
                comment: hex(0x555a65),
                keyword: hex(0x5138e7),
                string: hex(0x116530),
                string_special: hex(0x0c6178),
                number: hex(0x7f4e06),
                type_name: hex(0x7e22ce),
                function: hex(0x134ed0),
                property: hex(0xae1655),
                variable: hex(0x303035),
                punctuation: hex(0x52525b),
                markup_heading: hex(0x514db0),
                markup_raw: hex(0x0f5b41),
                markup_link: hex(0x8d2f58),
                markup_reference: hex(0x7c451e),
                invalid: hex(0xb11b1b),
            }
        }
    }

    /// The terminal's ANSI 0–15 (`ANSI_DARK` / `ANSI_LIGHT`).
    pub fn ansi(&self) -> [Hsla; 16] {
        let table: [u32; 16] = if self.dark {
            [
                0x242424, 0xf87171, 0x4ade80, 0xfacc15, 0x60a5fa, 0xc084fc, 0x22d3ee, 0xd4d4d8,
                0x52525b, 0xfca5a5, 0x86efac, 0xfde047, 0x93c5fd, 0xd8b4fe, 0x67e8f9, 0xfafafa,
            ]
        } else {
            [
                0x1f1f1f, 0xdc2626, 0x16a34a, 0xb45309, 0x2563eb, 0x9333ea, 0x0e7490, 0x3f3f46,
                0x71717a, 0xb91c1c, 0x15803d, 0x92400e, 0x1d4ed8, 0x7e22ce, 0x155e75, 0x18181b,
            ]
        };
        table.map(hex)
    }

    // ---- ink tiers (Pink `theme::ink` / `hairline` / `wash` / `scrim`) ----
    // Alphas are quoted in dark-mode terms; the light value is derived.

    /// Translucent fill ink for plates and states: white on dark, black on
    /// light at the same alpha.
    pub fn ink(&self, alpha: f32) -> Hsla {
        if self.dark {
            hsla(0.0, 0.0, 1.0, alpha)
        } else {
            hsla(0.0, 0.0, 0.0, alpha)
        }
    }

    /// Hairline ink for borders, dividers and rings: white on dark; black on
    /// light at 1.35× the alpha, capped at 0.5.
    pub fn hairline(&self, alpha: f32) -> Hsla {
        if self.dark {
            hsla(0.0, 0.0, 1.0, alpha)
        } else {
            hsla(0.0, 0.0, 0.0, (alpha * INK_HAIRLINE_SCALE).min(0.5))
        }
    }

    /// A softened state wash: `hsl(0 0% 92%)` on dark, `hsl(0 0% 10%)` on
    /// light.
    pub fn wash(&self, alpha: f32) -> Hsla {
        if self.dark {
            hsla(0.0, 0.0, 0.92, alpha)
        } else {
            hsla(0.0, 0.0, 0.10, alpha)
        }
    }

    /// A modal backdrop at `alpha_dark`: black, scaled to `0.32·α/0.6` on
    /// light. `t.scrim(MODAL_SCRIM)` is the dialog scrim.
    pub fn scrim(&self, alpha_dark: f32) -> Hsla {
        if self.dark {
            hsla(0.0, 0.0, 0.0, alpha_dark)
        } else {
            hsla(0.0, 0.0, 0.0, 0.32 * (alpha_dark / SCRIM_ALPHA_DARK))
        }
    }

    /// The recessed band behind a palette or picker header/footer strip
    /// (Pink `theme::band()`): black 16% / 4.5%.
    pub fn band(&self) -> Hsla {
        hsla(0.0, 0.0, 0.0, if self.dark { 0.16 } else { 0.045 })
    }

    /// An interaction wash from the [`FillTier`] ladder, tinted by the text
    /// hue (saturation capped at 0.35) at lightness 0.92 dark / 0.10 light.
    pub fn fill(&self, tier: FillTier) -> Hsla {
        let (h, s) = fill_tint(self.text);
        hsla(
            h,
            s,
            if self.dark { 0.92 } else { 0.10 },
            tier.alpha(self.dark),
        )
    }

    /// The separator hairline: text-tinted 8% (14% `strong`), scaled 1.35×
    /// on light like [`hairline`](Self::hairline).
    pub fn separator(&self, strong: bool) -> Hsla {
        separator_for(self.dark, fill_tint(self.text), strong)
    }

    // ---- selection and hover recipes ----

    /// Chrome hover (sidebar rows, tabs, titlebar buttons): `fill(Tertiary)`.
    pub fn glass_hover(&self) -> Hsla {
        self.fill(FillTier::Tertiary)
    }

    /// Sidebar row hover: `fill(Tertiary)`.
    pub fn sidebar_hover_bg(&self) -> Hsla {
        self.fill(FillTier::Tertiary)
    }

    /// Selected chrome rows and tabs: `fill(Primary)`, no ring.
    pub fn glass_selected_bg(&self) -> Hsla {
        self.fill(FillTier::Primary)
    }

    /// A selected or keyboard-active row inside a floating card: `wash(0.11)`
    /// dark, `wash(0.06)` light.
    pub fn card_selected_bg(&self) -> Hsla {
        self.wash(if self.dark { 0.11 } else { 0.06 })
    }

    /// The pointer's wash on a menu row: the selected wash at 60%.
    pub fn menu_hover_bg(&self) -> Hsla {
        let selected = self.card_selected_bg();
        Hsla {
            a: selected.a * 0.6,
            ..selected
        }
    }

    /// The user message bubble plate: `wash(0.08)` / `wash(0.065)`.
    pub fn user_bubble_bg(&self) -> Hsla {
        self.wash(if self.dark { 0.08 } else { 0.065 })
    }

    /// The user bubble's 1 px edge, light only: `hairline(0.06)`.
    pub fn user_bubble_edge(&self) -> Option<Hsla> {
        (!self.dark).then(|| self.hairline(0.06))
    }

    /// Tab chip fills `[rest, hover, active]`.
    pub fn tab_chip_washes(&self) -> [Hsla; 3] {
        [hsla(0.0, 0.0, 0.0, 0.0), self.wash(0.05), self.wash(0.09)]
    }

    /// The focused group's active tab: accent 16% / 10%.
    pub fn tab_focused_fill(&self) -> Hsla {
        self.accent.opacity(if self.dark { 0.16 } else { 0.10 })
    }

    /// A disabled control's glyph: `text_muted` at 35%.
    pub fn disabled_glyph(&self) -> Hsla {
        self.text_muted.opacity(0.35)
    }

    /// Purely decorative ink (the "·" between meta fragments): never for a
    /// label a user must read.
    pub fn decorative_muted(&self, dark_opacity: f32) -> Hsla {
        if self.dark {
            self.text_muted.opacity(dark_opacity)
        } else {
            self.text_muted
        }
    }

    /// GPUI scrollbar thumbs `[rest, hover, active]`: text 30 / 42 / 55%.
    pub fn scrollbar_thumbs(&self) -> [Hsla; 3] {
        [
            self.text.opacity(0.30),
            self.text.opacity(0.42),
            self.text.opacity(0.55),
        ]
    }

    // ---- Pink's frosted tints (translucent, as Pink paints them) ----

    /// Pink's frosted tints for this palette, memoized by
    /// [`palette_key`](Self::palette_key) (paint asks for them constantly).
    fn frost(&self) -> Frost {
        let key = self.palette_key();
        if let Some(hit) = FROST.with_borrow(|cache| cache.get(&key).copied()) {
            return hit;
        }
        let frost = self.derive_frost();
        FROST.with_borrow_mut(|cache| {
            if cache.len() > 64 {
                cache.clear();
            }
            cache.insert(key, frost);
        });
        frost
    }

    fn derive_frost(&self) -> Frost {
        let backdrop = self.adverse_backdrop();
        let base = if self.dark {
            GLASS_ALPHA
        } else {
            GLASS_ALPHA_LIGHT
        };
        let glass = self
            .surface
            .opacity(self.contrast_checked_glass_alpha(base));
        // The desktop behind the window, at its worst for this appearance.
        let window = flatten(glass, backdrop);
        let overlay_base = if self.dark { 0.50 } else { 0.85 };
        let overlay = self
            .surface_overlay
            .opacity(self.contrast_checked_tint_alpha(
                self.surface_overlay,
                overlay_base,
                backdrop,
            ));
        let input_base = if self.dark { self.input_bg.a } else { 0.30 };
        let input = self.input_bg.opacity(self.contrast_checked_tint_alpha(
            self.input_bg,
            input_base,
            window,
        ));
        let card =
            self.surface
                .opacity(self.contrast_checked_tint_alpha(self.surface, 0.40, window));
        Frost {
            glass,
            overlay,
            input,
            card,
            composer: self.composer_sidebar_tint_for(glass),
        }
    }

    /// Frost coverage over the adverse desktop (Pink
    /// `contrast_checked_glass_alpha`): raised from `base` until primary
    /// text clears its target on the chrome and everything derived from it
    /// (hover and selected washes, the canvas, the active-tab wash, the code
    /// plate), and muted text 3:1 on the chrome itself.
    fn contrast_checked_glass_alpha(&self, base: f32) -> f32 {
        let backdrop = self.adverse_backdrop();
        // Light frost is checked against the darker recessed plane the
        // canvas used to be (Pink audit SH-07).
        let canvas_fill = if self.dark {
            self.canvas_fill()
        } else {
            self.ink(0.035)
        };
        let tab_wash = self.wash(0.09);
        let code_plate = self.ink(0.035);
        for step in 0..=20 {
            let alpha = base + (1.0 - base) * step as f32 / 20.0;
            let chrome = flatten(self.surface.opacity(alpha), backdrop);
            let canvas = flatten(canvas_fill, chrome);
            let reads = [
                chrome,
                flatten(self.element_hover, chrome),
                flatten(self.element_active, chrome),
                canvas,
                flatten(tab_wash, canvas),
                flatten(code_plate, canvas),
            ]
            .iter()
            .all(|surface| painted_contrast(self.text, *surface) >= TEXT_CONTRAST_TARGET);
            if reads && painted_contrast(self.text_muted, chrome) >= 3.0 {
                return alpha;
            }
        }
        1.0
    }

    /// A frosted tint's coverage, raised from `base` only as far as primary
    /// text needs 4.5:1 and muted text 3:1 over `backdrop` (Pink
    /// `contrast_checked_tint_alpha`; `opacity` multiplies, as in GPUI).
    fn contrast_checked_tint_alpha(&self, tint: Hsla, base: f32, backdrop: Hsla) -> f32 {
        for step in 0..=20 {
            let alpha = base + (1.0 - base) * step as f32 / 20.0;
            let composite = flatten(tint.opacity(alpha), backdrop);
            if painted_contrast(self.text, composite) >= 4.5
                && painted_contrast(self.text_muted, composite) >= 3.0
            {
                return alpha;
            }
        }
        1.0
    }

    /// Pink `composer_sidebar_tint`: the 15% tint that moves the blurred
    /// canvas toward the right-pane tone, solved in RGB.
    fn composer_sidebar_tint_for(&self, glass: Hsla) -> Hsla {
        let target = if glass.a < 1.0 {
            flatten(self.bg.opacity(0.4), flatten(glass, self.bg))
        } else {
            self.bg
        };
        let canvas = flatten(glass, self.bg);
        let canvas = hsl_to_rgb(canvas.h, canvas.s, canvas.l);
        let target = hsl_to_rgb(target.h, target.s, target.l);
        let mut alpha: f32 = 0.60;
        for (base, desired) in canvas.into_iter().zip(target) {
            let needed = if desired > base {
                (desired - base) / (1.0 - base).max(f32::EPSILON)
            } else {
                (base - desired) / base.max(f32::EPSILON)
            };
            alpha = alpha.max(needed);
        }
        let rgb: [f32; 3] = std::array::from_fn(|i| {
            ((target[i] - canvas[i] * (1.0 - alpha)) / alpha).clamp(0.0, 1.0)
        });
        let (h, s, l) = rgb_to_hsl(rgb[0], rgb[1], rgb[2]);
        hsla(h, s, l, 0.15)
    }

    fn adverse_backdrop(&self) -> Hsla {
        if self.dark { grey(0xff) } else { grey(0) }
    }

    /// Pink's `chrome_surface`: the glass over the adverse desktop (white
    /// under dark, black under light), the worst backdrop text can meet.
    fn adverse_chrome(&self) -> Hsla {
        flatten(self.frost().glass, self.adverse_backdrop())
    }

    // ---- surfaces, baked over the canvas ----
    //
    // Each is Pink's frosted tint flattened over `bg`, the canvas behind every
    // demo, standing in for the blur GPUI here can't paint. All are opaque.

    /// The shell chrome and sidebar: Pink's glass (`surface` at 80% / 94%)
    /// over the canvas (#0c0c0c / #f1f4f7).
    pub fn chrome_surface(&self) -> Hsla {
        flatten(self.frost().glass, self.bg)
    }

    /// The workbench canvas tint over the chrome: `bg` at 55% / 96%.
    pub fn canvas_fill(&self) -> Hsla {
        self.bg.opacity(if self.dark { 0.55 } else { 0.96 })
    }

    /// The canvas as composited over the chrome (#090909 / #fbfcfe).
    pub fn canvas(&self) -> Hsla {
        flatten(self.canvas_fill(), self.chrome_surface())
    }

    /// Where content dissolves at an overflowing edge: `bg` at 60% / 55%.
    pub fn overflow_fade(&self) -> Hsla {
        self.bg.opacity(if self.dark { 0.6 } else { 0.55 })
    }

    /// A floating card's tint (Pink `glass_overlay`: `surface_overlay` at
    /// 67.5% / 85%) over the canvas (#141415 / #fefefe).
    pub fn glass_overlay(&self) -> Hsla {
        flatten(self.frost().overlay, self.bg)
    }

    /// Every tooltip, hover card and toast's fill. Pink paints these opaque
    /// in every treatment: the overlay tint over the chrome as composited
    /// (Pink `tooltip_surface`, the "overlay" reading surface), #262627 /
    /// #fbfbfc.
    pub fn tooltip_surface(&self) -> Hsla {
        flatten(self.frost().overlay, self.adverse_chrome())
    }

    /// Input plates (fields, the queue tray in Pink's own words): Pink's
    /// frosted `input_glass_bg` (`input_bg` at 52% / white 30%) over the
    /// canvas (#1e1e20 / #fcfdfe). The composer pill itself is
    /// [`surface_bg`](Self::surface_bg) under frost, as in Pink.
    pub fn input_glass_bg(&self) -> Hsla {
        flatten(self.frost().input, self.bg)
    }

    /// The popover, menu, dialog, palette and composer plate (Pink
    /// `popover::surface_bg`, frosted: `composer_sidebar_tint`, a 15% tint
    /// over the blur) over the canvas (#060606 / #fbfcfe). A frosted card
    /// reads as the window behind it; its hairline and shadows lift it.
    pub fn surface_bg(&self) -> Hsla {
        flatten(self.frost().composer, self.bg)
    }

    /// Settings section cards: Pink's frosted `card_glass_bg` (`surface` at
    /// 40%) over the canvas (#080808 / #f6f8fb).
    pub fn card_glass_bg(&self) -> Hsla {
        flatten(self.frost().card, self.bg)
    }

    /// The terminal panel: Pink thins the terminal background to 40% on
    /// glass; here baked over the chrome.
    pub fn terminal_panel_fill(&self) -> Hsla {
        flatten(self.terminal_bg.opacity(0.4), self.chrome_surface())
    }

    /// The opaque colors text sits on, as Pink composites them under frost:
    /// the glass over the adverse desktop (white under dark, black under
    /// light), its hover and selected washes, the canvas, the code plate,
    /// raised chips, the floating, input and settings-card tints over that
    /// chrome, and dialogs over their scrim (Pink `Theme::reading_surfaces`).
    /// Derived foregrounds are floored on the worst of them, so they read
    /// over any backdrop, the baked canvas included.
    pub fn reading_surfaces(&self) -> [Hsla; 10] {
        let frost = self.frost();
        let chrome = self.adverse_chrome();
        let canvas = flatten(self.canvas_fill(), chrome);
        [
            chrome,
            flatten(self.element_hover, chrome),
            flatten(self.element_active, chrome),
            canvas,
            flatten(self.ink(0.035), canvas),
            flatten(self.surface_raised, chrome),
            flatten(frost.overlay, chrome),
            flatten(frost.input, chrome),
            flatten(frost.card, chrome),
            flatten(
                self.surface_dialog,
                flatten(self.scrim(SCRIM_ALPHA_DARK), chrome),
            ),
        ]
    }

    /// The tokens text inside a floating card reads (Pink
    /// `Theme::for_popup`): under frost, muted and faint become the text
    /// color at 64% and 48%, floored on the card's tint over the adverse
    /// chrome, so the primary > muted > hint ladder stays translucent.
    /// Menu headings, kbd hints and key hints paint from it.
    pub fn for_popup(&self) -> Pink {
        let card = flatten(self.frost().overlay, self.adverse_chrome());
        let mut popup = self.clone();
        popup.text_muted = legible_on(
            self.text.opacity(0.64),
            self.text,
            &[card],
            TEXT_CONTRAST_TARGET,
        );
        popup.text_faint = legible_on(
            self.text.opacity(0.48),
            self.text,
            &[card],
            TEXT_CONTRAST_TARGET,
        );
        popup
    }

    // ---- contrast floors (Pink `Theme::readable_*`) ----

    /// `color` as asked when it clears `min` on every reading surface;
    /// otherwise raised (alpha, then lightness) until it does.
    pub fn legible(&self, color: Hsla, min: f32) -> Hsla {
        self.memo(&[color], min, || {
            legible_on(color, self.text, &self.reading_surfaces(), min)
        })
    }

    /// Secondary text (Pink `Theme::readable_muted`). Dark keeps the quieter
    /// opacity asked for, floored at the text target; light gives the same
    /// request its own step between the token and the floor
    /// ([`Self::light_tier`]). `readable_muted(0.95)` is the default glyph
    /// and ghost-label tone.
    pub fn readable_muted(&self, dark_opacity: f32) -> Hsla {
        if !self.dark {
            return self.light_tier(self.text_muted, dark_opacity);
        }
        self.legible(self.text_muted.opacity(dark_opacity), TEXT_CONTRAST_TARGET)
    }

    /// Faint text (hints, placeholders) is still text: held to the floor
    /// (Pink `Theme::readable_faint`).
    pub fn readable_faint(&self, dark_opacity: f32) -> Hsla {
        if !self.dark {
            return self.light_tier(self.text_faint, dark_opacity);
        }
        self.legible(self.text_faint.opacity(dark_opacity), TEXT_CONTRAST_TARGET)
    }

    /// Light appearance's answer to a dark-tuned opacity (Pink
    /// `Theme::light_tier`, UI rethink bug 18). Thinning dark ink over a
    /// light canvas drops below the floor almost at once, so the opacity
    /// instead picks a contrast between the token's own and the floor: 1
    /// keeps the token, 0.5 or less sits on the floor, and steps in between
    /// stay distinct and ordered. Hue and saturation are kept.
    fn light_tier(&self, token: Hsla, dark_opacity: f32) -> Hsla {
        let token = self.legible(token, TEXT_CONTRAST_TARGET);
        if dark_opacity >= 0.999 {
            return token;
        }
        let position = ((dark_opacity - 0.5) / 0.5).clamp(0.0, 1.0);
        let ceiling = self.worst_contrast(token).max(TEXT_CONTRAST_TARGET);
        let target = TEXT_CONTRAST_TARGET + (ceiling - TEXT_CONTRAST_TARGET) * position;
        // From the canvas side, the flooring walk darkens until it reads at
        // `target`, landing on the lightest tone that does.
        self.legible(
            Hsla {
                l: 0.98,
                a: 1.0,
                ..token
            },
            target,
        )
    }

    /// The lowest contrast `color` reaches across [`Self::reading_surfaces`]
    /// (Pink `Theme::worst_contrast`).
    pub fn worst_contrast(&self, color: Hsla) -> f32 {
        self.reading_surfaces()
            .iter()
            .map(|surface| painted_contrast(color, *surface))
            .fold(f32::MAX, f32::min)
    }

    /// Semantic copy (status words, selected chip labels) in `tone`, floored
    /// at the text target.
    pub fn readable_tone(&self, tone: Hsla, dark_opacity: f32) -> Hsla {
        let base = if self.dark {
            tone.opacity(dark_opacity)
        } else {
            tone
        };
        self.legible(base, TEXT_CONTRAST_TARGET)
    }

    /// A meaningful glyph in `tone`, floored at the non-text target.
    pub fn readable_icon(&self, tone: Hsla) -> Hsla {
        self.legible(tone, NON_TEXT_CONTRAST_TARGET)
    }

    /// `color` painted on a translucent `backing` (a pill's own wash),
    /// floored on that wash over every reading surface.
    pub fn readable_on(&self, color: Hsla, backing: Hsla, min: f32) -> Hsla {
        self.readable_on_stacks(color, &[&[backing]], min)
    }

    /// `color` floored on each stack of translucent layers (bottom first)
    /// composited over every reading surface.
    pub fn readable_on_stacks(&self, color: Hsla, stacks: &[&[Hsla]], min: f32) -> Hsla {
        let mut key = vec![color];
        for stack in stacks {
            // A separator keeps [a][b] and [a, b] distinct.
            key.push(Hsla {
                h: -1.0,
                s: -1.0,
                l: -1.0,
                a: stack.len() as f32,
            });
            key.extend_from_slice(stack);
        }
        self.memo(&key, min, || {
            let surfaces = self.reading_surfaces();
            let layered: Vec<Hsla> = stacks
                .iter()
                .flat_map(|stack| {
                    surfaces.iter().map(move |surface| {
                        stack
                            .iter()
                            .fold(*surface, |under, layer| flatten(*layer, under))
                    })
                })
                .collect();
            legible_on(color, self.text, &layered, min)
        })
    }

    /// `color` floored where the composer pill paints it (Pink
    /// `readable_on_composer`): on the pill's frosted tint over every reading
    /// surface. For the composer's controls row (the context share and ring).
    pub fn readable_on_composer(&self, color: Hsla, min: f32) -> Hsla {
        self.readable_on_stacks(color, &[&[self.frost().composer]], min)
    }

    /// The keyboard focus ring color: the accent floored at 3.1:1 on every
    /// reading surface and tab chip fill (#bbb3fa dark, #5b43e8 light).
    pub fn focus_ring(&self) -> Hsla {
        let [rest, hover, active] = self.tab_chip_washes();
        let focused = self.tab_focused_fill();
        self.readable_on_stacks(
            self.accent,
            &[&[rest], &[hover], &[active], &[focused]],
            NON_TEXT_CONTRAST_TARGET,
        )
    }

    /// An accent check or indicator on a selected menu row, resting or
    /// hovered.
    pub fn selected_row_icon(&self, tone: Hsla) -> Hsla {
        let selected = self.card_selected_bg();
        self.readable_on_stacks(
            tone,
            &[&[selected], &[selected, self.ink(0.05)]],
            NON_TEXT_CONTRAST_TARGET,
        )
    }

    /// Label on the Danger plate: `on_accent` when it reads on
    /// `danger_strong`, else white or black (black dark, white light).
    pub fn on_danger(&self) -> Hsla {
        on_fill(self.on_accent, &[self.danger_strong])
    }

    /// The default tone of a glyph with no other role: `readable_muted(0.95)`
    /// (Pink `current_glyph_tone`).
    pub fn glyph_tone(&self) -> Hsla {
        self.readable_muted(0.95)
    }

    // ---- elevation ----

    /// Floating menus, popovers and toasts: a soft ambient drop plus a tight
    /// contact shadow (Pink `menu_shadows`). The kit's cards are baked
    /// opaque, so the shadow never stains through them.
    pub fn menu_shadows(&self) -> Vec<BoxShadow> {
        let (ambient, contact) = if self.dark {
            (0.45, 0.30)
        } else {
            (0.16, 0.10)
        };
        vec![
            shadow(0., 10., 24., -6., hsla(0., 0., 0., ambient)),
            shadow(0., 1., 2., 0., hsla(0., 0., 0., contact)),
        ]
    }

    /// Pink's frosted-card elevation (`frost::paint_elevation`, audit
    /// CP-16): eight 1 px rings outside the card, a crisp contact rim (black
    /// 42% / 10%) then an ambient falloff (`20% × (1 − (d − 1)/8)²`, light
    /// 5.5%) drifting 0.45 px lower per ring. Painted here as crisp shadows
    /// of spread `d`, each alpha solved so the stack shows ring `d`'s alpha
    /// in the band `d` px out. Floating cards (popovers, menus, dialogs,
    /// palettes) carry them above their own shadows.
    pub fn elevation_rings(&self) -> Vec<BoxShadow> {
        const RINGS: usize = 8;
        let (contact, ambient) = if self.dark {
            (0.42, 0.20)
        } else {
            (0.10, 0.055)
        };
        let ring = |d: usize| -> f32 {
            match d {
                1 => contact,
                d if d <= RINGS => {
                    let falloff = 1.0 - (d as f32 - 1.0) / RINGS as f32;
                    ambient * falloff * falloff
                }
                _ => 0.0,
            }
        };
        (1..=RINGS)
            .map(|d| {
                // Shadows d..=8 all cover the band d px out; this one adds
                // what the outer ones leave.
                let alpha = 1.0 - (1.0 - ring(d)) / (1.0 - ring(d + 1));
                let drift = (d as f32 - 1.0) * 0.45;
                shadow(0., drift, 0., d as f32, hsla(0., 0., 0., alpha))
            })
            .collect()
    }

    /// Tooltips and hover cards: one step below menus (Pink
    /// `tooltip_shadows`).
    pub fn tooltip_shadows(&self) -> Vec<BoxShadow> {
        let alpha = if self.dark { 0.35 } else { 0.12 };
        vec![shadow(0., 4., 10., 0., hsla(0., 0., 0., alpha))]
    }

    /// The selection outline for rows and chips inside a floating card: a
    /// 1 px inset ring, white 9% dark, a flat black 7% light (Pink
    /// `card_selected_shadows` / `glass_selected_shadows`).
    pub fn card_selected_shadows(&self) -> Vec<BoxShadow> {
        let color = if self.dark {
            self.hairline(0.09)
        } else {
            hsla(0.0, 0.0, 0.0, 0.07)
        };
        vec![inset_ring(1.0, color)]
    }

    /// The focus ring as a shadow list, for elements that paint it
    /// themselves instead of through `.focus_visible(..)`.
    pub fn focus_ring_shadows(&self) -> Vec<BoxShadow> {
        vec![inset_ring(2.0, self.focus_ring())]
    }

    /// Kept for older call sites: same as [`menu_shadows`](Self::menu_shadows).
    pub fn menu_shadow(&self) -> Vec<BoxShadow> {
        self.menu_shadows()
    }

    /// Kept for older call sites: same as
    /// [`tooltip_shadows`](Self::tooltip_shadows).
    pub fn tooltip_shadow(&self) -> Vec<BoxShadow> {
        self.tooltip_shadows()
    }

    /// Kept for older call sites. Pink has no card shadow: in-flow cards are
    /// never shadowed (`settings::widgets::section_card`, transcript code
    /// surfaces), so this is empty. Floating cards use
    /// [`menu_shadows`](Self::menu_shadows); inline auth cards
    /// [`shadow_lg`].
    pub fn card_shadow(&self) -> Vec<BoxShadow> {
        Vec::new()
    }

    /// Paint-time memo keyed by the palette, the inputs and the floor.
    fn memo(&self, inputs: &[Hsla], min: f32, derive: impl FnOnce() -> Hsla) -> Hsla {
        let mut bits = Vec::with_capacity(inputs.len() * 4 + 1);
        for color in inputs {
            bits.extend([
                color.h.to_bits(),
                color.s.to_bits(),
                color.l.to_bits(),
                color.a.to_bits(),
            ]);
        }
        bits.push(min.to_bits());
        let key = (self.palette_key(), bits);
        if let Some(hit) = MEMO.with_borrow(|cache| cache.get(&key).copied()) {
            return hit;
        }
        let derived = derive();
        MEMO.with_borrow_mut(|cache| {
            if cache.len() > 4096 {
                cache.clear();
            }
            cache.insert(key, derived);
        });
        derived
    }

    /// A key for everything the reading surfaces depend on: the frosted
    /// tints' coverage reads `text` and `text_muted` (Pink `palette_key`).
    fn palette_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.dark.hash(&mut hasher);
        for color in [
            self.text,
            self.text_muted,
            self.bg,
            self.surface,
            self.surface_raised,
            self.surface_card,
            self.surface_dialog,
            self.surface_overlay,
            self.input_bg,
            self.element_hover,
            self.element_active,
        ] {
            for part in [color.h, color.s, color.l, color.a] {
                part.to_bits().hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

/// Hue and capped saturation of the text color: fills carry the palette's
/// tint (Pink `fill_tint`).
fn fill_tint(text: Hsla) -> (f32, f32) {
    (text.h, text.s.min(0.35))
}

fn separator_for(dark: bool, (h, s): (f32, f32), strong: bool) -> Hsla {
    let alpha = if strong { 0.14 } else { 0.08 };
    if dark {
        hsla(h, s, 0.92, alpha)
    } else {
        hsla(h, s, 0.10, alpha * INK_HAIRLINE_SCALE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb8(c: Hsla) -> u32 {
        let c: gpui::Rgba = c.into();
        let ch = |v: f32| (v * 255.0).round() as u32;
        (ch(c.r) << 16) | (ch(c.g) << 8) | ch(c.b)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.002
    }

    /// The authored model values Pink hardens (`builtins.rs` `variant()`),
    /// on this appearance's palette.
    fn authored(dark: bool) -> Pink {
        let mut a = Pink::for_mode(dark);
        let (muted, faint) = if dark {
            (hex(0xa9a9ae), hex(0x85858a))
        } else {
            (hex(0x445064), hex(0x586579))
        };
        a.text_muted = muted;
        a.text_dim = muted;
        a.text_faint = faint;
        if dark {
            a.danger_muted = hex(0xf49293);
            a.warning_muted = hex(0xf6d34a);
            a.success_muted = hex(0x61d8ad);
            a.border_strong = hexa(0xffffff, 0x2e);
        } else {
            a.danger_muted = hex(0xa72629);
            a.warning_muted = hex(0x815312);
            a.success_muted = hex(0x186a3a);
            a.border_strong = hexa(0x000000, 0x38);
        }
        a.code_text = a.accent;
        a
    }

    /// Pink's `harden_composited_foregrounds` (two passes, because the
    /// frosted tints' coverage reads `text_muted`), `harden_content_tones`
    /// and the inline-code floor, verbatim, over the kit's reading surfaces.
    fn pink_hardening(authored: &Pink) -> Pink {
        let mut h = authored.clone();
        for _ in 0..2 {
            h.text_muted = h.legible(h.text_muted, TEXT_CONTRAST_TARGET);
            h.text_dim = h.legible(h.text_dim, TEXT_CONTRAST_TARGET);
            h.text_faint = h.legible(h.text_faint, TEXT_CONTRAST_TARGET);
            h.danger_muted = h.legible(h.danger_muted, TEXT_CONTRAST_TARGET);
            h.warning_muted = h.legible(h.warning_muted, TEXT_CONTRAST_TARGET);
            h.success_muted = h.legible(h.success_muted, TEXT_CONTRAST_TARGET);
            h.border_strong = h.legible(h.border_strong, NON_TEXT_CONTRAST_TARGET);
        }
        let canvas = flatten(h.canvas_fill(), h.adverse_chrome());
        let content = [canvas, flatten(h.ink(0.035), canvas)];
        let worst = |color: Hsla| {
            content
                .iter()
                .map(|surface| painted_contrast(color, *surface))
                .fold(f32::MAX, f32::min)
        };
        h.content_faint = legible_on(authored.text_faint, h.text, &content, TEXT_CONTRAST_TARGET);
        let secondary_floor = (worst(h.content_faint) * 1.35)
            .max(TEXT_CONTRAST_TARGET)
            .min(worst(h.text) * 0.75);
        h.content_muted = legible_on(authored.text_muted, h.text, &content, secondary_floor);
        let frost = h.frost();
        let plates = [
            flatten(frost.composer, canvas),
            flatten(frost.input, canvas),
        ];
        h.placeholder = legible_on(authored.text_faint, h.text, &plates, TEXT_CONTRAST_TARGET);
        let chip = [
            flatten(h.code_wash, canvas),
            flatten(h.code_wash, flatten(h.ink(0.035), canvas)),
        ];
        h.code_text = legible_on(authored.code_text, h.text, &chip, TEXT_CONTRAST_TARGET);
        h
    }

    #[test]
    fn neutral_scale_matches_tailwind() {
        // neutral-950 is #0a0a0a, neutral-200 is #e5e5e5
        assert_eq!(rgb8(neutral(0.145)), 0x0a0a0a);
        assert_eq!(rgb8(neutral(0.922)), 0xe5e5e5);
    }

    /// Pink's frosted reading surfaces and the plates the kit bakes over
    /// the canvas, against the verbatim port of Pink's resolution
    /// (`chrome`, `chrome-hover`, `chrome-selected`, `canvas`, `code`,
    /// `raised`, `overlay`, `input`, `card`, `dialog`).
    #[test]
    fn surfaces_match_pink_runtime() {
        let (d, l) = (Pink::dark(), Pink::light());
        let reading = |t: &Pink| t.reading_surfaces().map(rgb8);
        assert_eq!(
            reading(&d),
            [
                0x3d3d3d, 0x535353, 0x4b495f, 0x1f1f1f, 0x272727, 0x343438, 0x262627, 0x39393b,
                0x2a2a2a, 0x151516,
            ]
        );
        assert_eq!(
            reading(&l),
            [
                0xe2e4e8, 0xd4d7db, 0xd4d4e8, 0xfafbfd, 0xf1f2f4, 0xe7ecf2, 0xfbfbfc, 0xeaecef,
                0xe7eaee, 0xfefefe,
            ]
        );
        // The frosted tints' coverage: glass 80/94%, overlay 67.5/85%,
        // input 52.1/30%, settings card 40%, popover 15%.
        for (t, alphas) in [
            (&d, [0.80, 0.675, 0.5207, 0.40, 0.15]),
            (&l, [0.94, 0.85, 0.30, 0.40, 0.15]),
        ] {
            let f = t.frost();
            for (tint, alpha) in [f.glass, f.overlay, f.input, f.card, f.composer]
                .into_iter()
                .zip(alphas)
            {
                assert!(close(tint.a, alpha), "{tint:?}");
            }
        }
        // Tooltips are opaque in Pink: the overlay tint over the chrome.
        assert_eq!(rgb8(d.tooltip_surface()), 0x262627);
        assert_eq!(rgb8(l.tooltip_surface()), 0xfbfbfc);
        // Baked over the canvas: no grey plate behind popovers or cards.
        for (t, chrome, popover, input, card) in [
            (&d, 0x0c0c0c, 0x060606, 0x1e1e20, 0x090909),
            (&l, 0xf1f4f7, 0xfafcfe, 0xfcfdfe, 0xf7f8fb),
        ] {
            assert_eq!(rgb8(t.chrome_surface()), chrome);
            assert_eq!(rgb8(t.surface_bg()), popover);
            assert_eq!(rgb8(t.input_glass_bg()), input);
            assert_eq!(rgb8(t.card_glass_bg()), card);
            for plate in [t.chrome_surface(), t.surface_bg(), t.card_glass_bg()] {
                assert_eq!(plate.a, 1.0);
            }
        }
        assert_eq!(rgb8(d.canvas()), 0x090909);
    }

    /// Every hardened token is Pink's derivation from the authored seeds
    /// over the frosted reading surfaces.
    #[test]
    fn hardened_tokens_are_pinks_derivation() {
        for t in [Pink::dark(), Pink::light()] {
            let h = pink_hardening(&authored(t.dark));
            for (name, derived, token) in [
                ("text_muted", h.text_muted, t.text_muted),
                ("text_dim", h.text_dim, t.text_dim),
                ("text_faint", h.text_faint, t.text_faint),
                ("danger_muted", h.danger_muted, t.danger_muted),
                ("warning_muted", h.warning_muted, t.warning_muted),
                ("success_muted", h.success_muted, t.success_muted),
                ("border_strong", h.border_strong, t.border_strong),
                ("content_muted", h.content_muted, t.content_muted),
                ("content_faint", h.content_faint, t.content_faint),
                ("placeholder", h.placeholder, t.placeholder),
                ("code_text", h.code_text, t.code_text),
            ] {
                assert_eq!(rgb8(derived), rgb8(token), "{name} {}", t.dark);
                // The exact floored values, not only their 8-bit rendering.
                assert!(
                    close(derived.l, token.l) && close(derived.a, token.a),
                    "{name} {}: {derived:?}",
                    t.dark
                );
            }
            // The resolved tokens are stable: hardening them again moves
            // nothing.
            for tone in [t.text_muted, t.text_faint, t.warning_muted, t.success_muted] {
                assert_eq!(t.legible(tone, TEXT_CONTRAST_TARGET), tone);
            }

            // `harden_syntax`: each seed floored on the reading surfaces and
            // the diff washes over the chrome and the canvas.
            let mut surfaces = t.reading_surfaces().to_vec();
            let chrome = t.adverse_chrome();
            let canvas = flatten(t.canvas_fill(), chrome);
            for tone in [t.diff_add, t.diff_del] {
                let wash = Hsla { a: 0.055, ..tone };
                surfaces.extend([flatten(wash, chrome), flatten(wash, canvas)]);
            }
            // [comment, keyword, string, attribute, number, type, function,
            // property, variable, punctuation, invalid] (`builtins.rs`).
            let seeds: [u32; 11] = if t.dark {
                [
                    0x92929a, 0x8b7cf6, 0x34d399, 0x22d3ee, 0xfacc15, 0xc084fc, 0x60a5fa, 0xf472b6,
                    0xe8e8ea, 0xa1a1aa, 0xf87171,
                ]
            } else {
                [
                    0x6b7280, 0x5b43e8, 0x15803d, 0x0e7490, 0xa16207, 0x7e22ce, 0x2563eb, 0xbe185d,
                    0x303035, 0x52525b, 0xb91c1c,
                ]
            };
            let syntax = t.syntax();
            let kit = [
                syntax.comment,
                syntax.keyword,
                syntax.string,
                syntax.string_special,
                syntax.number,
                syntax.type_name,
                syntax.function,
                syntax.property,
                syntax.variable,
                syntax.punctuation,
                syntax.invalid,
            ];
            // Stored as 8-bit hex: within one step per channel.
            let channels = |c: Hsla| {
                let c = rgb8(c);
                [c >> 16, (c >> 8) & 0xff, c & 0xff]
            };
            for (seed, kit) in seeds.into_iter().zip(kit) {
                let hardened = legible_on(hex(seed), t.text, &surfaces, TEXT_CONTRAST_TARGET);
                let near = channels(hardened)
                    .into_iter()
                    .zip(channels(kit))
                    .all(|(a, b)| a.abs_diff(b) <= 1);
                assert!(near, "#{seed:06x}: #{:06x}", rgb8(hardened));
            }
        }
    }

    /// Text contrast tiers (Pink `theme.rs` D38, bug 18). Light gives a
    /// quieter request its own step between the token and the floor instead
    /// of one grey for every request; nothing that is text drops under
    /// 4.5:1 anywhere; a quieter request never reads stronger; dark keeps
    /// the opacity it asks for. On the sidebar chrome the tiers land where
    /// Pink measured them: about 12, 6.5 and 5.5:1. Pink's lightness walk
    /// takes 24 steps, so neighbouring requests can share a step.
    #[test]
    fn light_text_tiers_stay_distinct_and_legible() {
        let l = Pink::light();
        let chrome = l.reading_surfaces()[0];
        let on_chrome = |c: Hsla| painted_contrast(c, chrome);
        let quieter = [0.95, 0.85, 0.7, 0.6, 0.5, 0.3];
        type Tier = fn(&Pink, f32) -> Hsla;
        for (name, tier) in [
            ("muted", Pink::readable_muted as Tier),
            ("faint", Pink::readable_faint as Tier),
        ] {
            let ratios: Vec<f32> = quieter.iter().map(|o| on_chrome(tier(&l, *o))).collect();
            for pair in ratios.windows(2) {
                assert!(
                    pair[0] >= pair[1],
                    "{name} reads stronger when quieter: {ratios:?}"
                );
            }
            for o in [1.0].iter().chain(&quieter) {
                let worst = l.worst_contrast(tier(&l, *o));
                assert!(worst >= TEXT_CONTRAST_MIN, "{name} {o}: {worst}");
            }
            // Secondary and tertiary requests are distinct steps.
            assert!(ratios[0] - ratios[4] > 0.5, "{name} collapses: {ratios:?}");
            // Hue survives, so tinted themes stay tinted.
            assert!(
                (tier(&l, 0.6).h - l.text_muted.h).abs() < 0.05,
                "{name} hue"
            );
        }
        let primary = on_chrome(l.text);
        let secondary = on_chrome(l.readable_muted(0.85));
        let tertiary = on_chrome(l.readable_faint(0.6));
        assert!(primary >= TEXT_CONTRAST_PRIMARY - 0.5, "{primary}");
        assert!(secondary >= TEXT_CONTRAST_SECONDARY, "{secondary}");
        assert!(
            (TEXT_CONTRAST_TERTIARY..TEXT_CONTRAST_SECONDARY).contains(&tertiary),
            "{tertiary}"
        );
        // Dark is unchanged: the opacity it asks for, floored.
        assert!(close(Pink::dark().readable_muted(0.95).a, 0.985));
    }

    /// Hit sizes and the titlebar cluster (Pink `surface_chrome.rs`,
    /// `shell.rs`) at Interface sizes 12, 16 and 20.
    #[test]
    fn hit_sizes_scale_within_the_ladder() {
        for (ui, control, icon, cluster) in [
            (12.0, 24.0, 12.0, 82.0),
            (16.0, 28.0, 16.0, 94.0),
            (20.0, 32.0, 18.0, 106.0),
        ] {
            let scale = ui / 16.0;
            assert_eq!(layout::titlebar_control_size(scale), control, "{ui}");
            assert!(chrome::scaled_control(chrome::CONTROL_S, scale) >= chrome::MIN_HIT);
            assert_eq!(chrome::scaled_icon(chrome::ICON_M, scale), icon, "{ui}");
            assert_eq!(layout::cluster_buttons_width(scale), cluster, "{ui}");
        }
    }

    /// Paint-time derivations, against the port's frosted dump.
    #[test]
    fn derived_paint_matches_pink() {
        let (d, l) = (Pink::dark(), Pink::light());
        assert_eq!(rgb8(d.focus_ring()), 0xbbb3fa);
        assert_eq!(rgb8(l.focus_ring()), 0x5b43e8);
        assert!(close(d.readable_muted(0.95).a, 0.985));
        assert_eq!(rgb8(d.readable_muted(0.6)), 0xc9c9cc);
        assert!(close(d.readable_muted(0.6).a, 1.0));
        assert_eq!(rgb8(d.readable_faint(0.6)), 0xccccce);
        // Light picks its own step for a quieter request (Pink bug 18):
        // no longer the bare token #4d586a.
        assert_eq!(rgb8(l.readable_faint(0.6)), 0x4f5b6d);
        assert_eq!(rgb8(l.readable_faint(1.0)), rgb8(l.text_faint));
        assert_eq!(rgb8(d.on_danger()), 0x000000);
        assert_eq!(rgb8(l.on_danger()), 0xffffff);
        assert_eq!(rgb8(d.fill(FillTier::Tertiary)), 0xeaeaec);
        assert_eq!(rgb8(l.fill(FillTier::Tertiary)), 0x14181f);
        assert!(close(l.separator(false).a, 0.108));
        assert!(close(l.hairline(0.5).a, 0.5));
        // `for_popup`: the text color at 64% / 48%, floored on the card.
        for (t, muted, faint) in [(&d, 0.64, 0.584), (&l, 0.64, 0.688)] {
            let popup = t.for_popup();
            assert_eq!(rgb8(popup.text_muted), rgb8(t.text));
            assert!(close(popup.text_muted.a, muted), "{:?}", popup.text_muted);
            assert!(close(popup.text_faint.a, faint), "{:?}", popup.text_faint);
        }
        // The frost elevation rings: the stacked shadows show ring d's alpha
        // in the band d px out (contact 0.42, then 0.153 … dark; 0.10,
        // 0.042 … light).
        for (t, rings) in [(&d, [0.42, 0.153, 0.1125]), (&l, [0.10, 0.0421, 0.0309])] {
            let shadows = t.elevation_rings();
            for (band, ring) in rings.into_iter().enumerate() {
                let covered = shadows
                    .iter()
                    .filter(|s| s.spread_radius >= px(band as f32 + 1.0))
                    .fold(0.0, |under, s| s.color.a + under * (1.0 - s.color.a));
                assert!(close(covered, ring), "band {band}: {covered}");
            }
        }
    }
}
