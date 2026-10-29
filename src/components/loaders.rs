//! Loaders – every Pink loader on one settings card, each shown where Pink
//! shows it (Pink: loaders.rs, the skeletons in popover.rs and the working
//! trailer in transcript.rs; the card is settings/widgets.rs
//! `section_card` with `option_card` previews, as on Pink's Appearance page).
//!
//! The card's rows group the loaders by job. "Working": the breathing orb in
//! the composer's Sending strip, the working line with its shimmering verb
//! and timer, the 2×3 glyph spinner in a running tool row and the mono
//! spinner in the sidebar's Reconnecting line. "Loading": the gradient matrix
//! over a loading pane, skeleton rows in a settings list, menu skeleton bars
//! in a picker and the ghost label in a composer chip. "Progress and brand":
//! the upload ring over an attachment, the boot splash with its fade, and
//! the Pink wave and mark loaders. Every preview paints the surface its
//! loader sits on in Pink. Pick a preview (click, or Tab then the arrow
//! keys) and the last row names its timing and the kit call.
//!
//! Everything loops on the shared pulse clock; under reduced motion each
//! loader rests on its still frame.
//!
//! The gradient matrix, the glyph and mono spinners, the upload ring and the
//! boot splash are the `pink-loaders` package (`components/loaders`), PINK's
//! loaders as a single-GPUI package the app pins (UI rethink decision 54).
//! The gallery compiles the package's own source here, unchanged, against
//! its GPUI; PINK compiles the same file against its patched one. The kit's
//! [`crate::pink::loaders`] builders draw through it, passing the pulse
//! clock's phase and the gradient-spin curve as PINK's adapters do.
//!
//! ```ignore
//! Loaders::new("loaders")
//!     .working(WorkingSample::new("Reading").object("ci.yml").secs(12))
//!     .upload_percent(64)
//!     .on_select(|kind, _, _| println!("{}", kind.symbol()))
//! ```

#[allow(dead_code)]
mod package {
    include!("../../components/loaders/src/lib.rs");
}
pub use package::*;

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AnyView, App, Context, Div, ElementId, Entity, FocusHandle, FontWeight, Hsla,
    IntoElement, KeyDownEvent, Role, SharedString, Task, Toggled, Window, div, hsla, prelude::*,
    px,
};

use crate::{
    motion::{self, PINK_PULSE},
    pink::{
        self, FillTier, Pink, layout,
        loaders::{
            self, LIVE_STATUS_OBJECT_CHARS, TOOL_LABEL_LINE_HEIGHT, TOOL_LABEL_SIZE,
            ellipsize_chars,
        },
        space,
    },
    story,
};

pub const NAME: &str = "Loaders";
pub const DESCRIPTION: &str = "Every Pink loader on one settings card: the breathing orb, the shimmering working line, glyph and mono spinners, the gradient matrix, skeletons, the upload ring, the boot splash and the Pink mark, each shown where the app shows it, with its timing a click away";

/// The card: Pink's settings column is 720 less its gutters; four previews
/// a row want a little more room than a settings page gives them.
const CARD_W: f32 = 880.;
/// The preview frame (Pink settings/widgets.rs `OPTION_CARD_HEIGHT`,
/// `OPTION_CARD_RADIUS`).
const PREVIEW_H: f32 = 148.;
const PREVIEW_RADIUS: f32 = 6.;
/// Previews in a row, and the gap between them (Pink `option_card_row`).
const PREVIEW_GAP: f32 = 16.;
/// An image attachment's thumbnail (Pink transcript.rs `ATT_THUMB_W`,
/// `ATT_THUMB_H`) and the ring over it (Pink: `upload_progress_ring(pct, 34)`).
const THUMB_W: f32 = 112.;
const THUMB_H: f32 = 80.;
const RING_D: f32 = 34.;
/// The tool-row glyph slot (Pink transcript.rs `ACTIVITY_ICON_SIZE`).
const ACTIVITY_ICON: f32 = 14.;
/// The composer chip's ghost label (Pink pickers.rs `skeleton_bar(56.0)`).
const GHOST_LABEL_W: f32 = 56.;
/// The splash's status line (Pink loaders.rs `splash_overlay`).
const SPLASH_LABEL: &str = "Setting up PINK environment";

/// One of Pink's loaders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoaderKind {
    BreathingOrb,
    WorkingLine,
    GlyphSpinner,
    MonoSpinner,
    GradientSpinner,
    SkeletonRows,
    MenuSkeleton,
    LabelGhost,
    UploadRing,
    BootSplash,
    PinkWave,
    PinkMark,
}

impl LoaderKind {
    pub const ALL: [LoaderKind; 12] = [
        LoaderKind::BreathingOrb,
        LoaderKind::WorkingLine,
        LoaderKind::GlyphSpinner,
        LoaderKind::MonoSpinner,
        LoaderKind::GradientSpinner,
        LoaderKind::SkeletonRows,
        LoaderKind::MenuSkeleton,
        LoaderKind::LabelGhost,
        LoaderKind::UploadRing,
        LoaderKind::BootSplash,
        LoaderKind::PinkWave,
        LoaderKind::PinkMark,
    ];

    /// The caption under its preview.
    pub fn name(self) -> &'static str {
        match self {
            Self::BreathingOrb => "Breathing orb",
            Self::WorkingLine => "Working line",
            Self::GlyphSpinner => "Glyph spinner",
            Self::MonoSpinner => "Mono spinner",
            Self::GradientSpinner => "Gradient matrix",
            Self::SkeletonRows => "Skeleton rows",
            Self::MenuSkeleton => "Menu skeleton",
            Self::LabelGhost => "Ghost label",
            Self::UploadRing => "Upload ring",
            Self::BootSplash => "Boot splash",
            Self::PinkWave => "Pink wave",
            Self::PinkMark => "Pink mark",
        }
    }

    /// Where Pink shows it.
    pub fn usage(self) -> &'static str {
        match self {
            Self::BreathingOrb => "The working line, the Sending strip",
            Self::WorkingLine => "Under the reply while a turn runs",
            Self::GlyphSpinner => "Running tools, chats, tabs and chips",
            Self::MonoSpinner => "The sidebar's Reconnecting line",
            Self::GradientSpinner => "Loading history, diffs and files",
            Self::SkeletonRows => "Settings lists while they load",
            Self::MenuSkeleton => "Pickers before their lists arrive",
            Self::LabelGhost => "A composer chip waiting for its label",
            Self::UploadRing => "Image attachments as they upload",
            Self::BootSplash => "The window while Pink starts",
            Self::PinkWave => "Pink's wave; no surface uses it yet",
            Self::PinkMark => "Pink's mark; no surface uses it yet",
        }
    }

    /// How it moves and what it is made of (Pink loaders.rs, motion.rs).
    pub fn spec(self) -> &'static str {
        match self {
            Self::BreathingOrb => {
                "A 6 px accent core in a 12 px halo that breathes from 16% to 34% once every 1.6 s."
            }
            Self::WorkingLine => {
                "The verb shimmers toward primary text every 3.4 s; each new status settles in over 180 ms, and the timer ticks in tabular figures."
            }
            Self::GlyphSpinner => {
                "Six 2 px cells in the accent's light, mid and deep tints; brightness chases round the ring every 750 ms."
            }
            Self::MonoSpinner => {
                "The same 750 ms ring chase in one muted tone, where an accent would pull focus."
            }
            Self::GradientSpinner => {
                "A 3×3 dot matrix; a wave rises from the bottom center every 750 ms, drawn at 15 fps."
            }
            Self::SkeletonRows => {
                "28 px bars that pulse between 35% and 75% every 2.4 s, each row 8% of a beat behind the last."
            }
            Self::MenuSkeleton => {
                "14 px bars at 42, 58, 48 and 66% of the menu's width, on the same staggered pulse."
            }
            Self::LabelGhost => {
                "An 11 px bar standing in for a label, on the same pulse, beside the glyph spinner."
            }
            Self::UploadRing => {
                "A white arc grows clockwise from 12 o'clock over a dimmed thumbnail, with the percent inside."
            }
            Self::BootSplash => {
                "The gradient matrix over the window; once ready, a 150 ms hold, then a 500 ms fade and 6 px lift."
            }
            Self::PinkWave => "Five cells pulse from 8% to full, 0.15 s apart, every 2.4 s.",
            Self::PinkMark => {
                "The mark's 34 cells, lit by a wave that runs from tail to head every 2.4 s."
            }
        }
    }

    /// The kit call that draws it.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::BreathingOrb => "loaders::breathing_orb",
            Self::WorkingLine => "loaders::working_status",
            Self::GlyphSpinner => "loaders::mini_glyph_spinner",
            Self::MonoSpinner => "loaders::mini_mono_spinner",
            Self::GradientSpinner => "loaders::gradient_spinner",
            Self::SkeletonRows => "loaders::skeleton_rows",
            Self::MenuSkeleton => "loaders::skeleton_menu_rows",
            Self::LabelGhost => "loaders::skeleton_bar",
            Self::UploadRing => "loaders::upload_progress_ring",
            Self::BootSplash => "loaders::splash_overlay",
            Self::PinkWave => "loaders::pink_loader",
            Self::PinkMark => "loaders::pink_mark_loader",
        }
    }
}

/// One row of the card: a title, an optional line under it, and the
/// loaders it previews (four fit a row).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoaderGroup {
    pub title: SharedString,
    pub description: Option<SharedString>,
    pub kinds: Vec<LoaderKind>,
}

impl LoaderGroup {
    pub fn new(
        title: impl Into<SharedString>,
        kinds: impl IntoIterator<Item = LoaderKind>,
    ) -> Self {
        Self {
            title: title.into(),
            description: None,
            kinds: kinds.into_iter().collect(),
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// The card's rows when none are given: every loader, grouped by job.
pub fn default_groups() -> Vec<LoaderGroup> {
    use LoaderKind::*;
    vec![
        LoaderGroup::new(
            "Working",
            [BreathingOrb, WorkingLine, GlyphSpinner, MonoSpinner],
        )
        .description("Something is running right now. One calm motion, never a ring."),
        LoaderGroup::new(
            "Loading",
            [GradientSpinner, SkeletonRows, MenuSkeleton, LabelGhost],
        )
        .description("Holds the place of content until it arrives, never a blank pane."),
        LoaderGroup::new(
            "Progress and brand",
            [UploadRing, BootSplash, PinkWave, PinkMark],
        )
        .description("An upload's progress, the window starting up, and the mark."),
    ]
}

/// What the working line says (Pink `LiveStatus`): a verb, what it acts on,
/// and the seconds since the turn began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingSample {
    pub verb: SharedString,
    pub object: Option<SharedString>,
    pub secs: i64,
}

impl WorkingSample {
    pub fn new(verb: impl Into<SharedString>) -> Self {
        Self {
            verb: verb.into(),
            object: None,
            secs: 0,
        }
    }

    pub fn object(mut self, object: impl Into<SharedString>) -> Self {
        self.object = Some(object.into());
        self
    }

    pub fn secs(mut self, secs: i64) -> Self {
        self.secs = secs;
        self
    }
}

/// Where the boot splash is in its life (Pink shell.rs: shown, fading on
/// `SPLASH_OUT`, then removed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SplashPhase {
    #[default]
    Showing,
    Fading,
    Gone,
}

type SelectHandler = Rc<dyn Fn(LoaderKind, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Loaders {
    id: ElementId,
    title: SharedString,
    description: Option<SharedString>,
    groups: Vec<LoaderGroup>,
    selected: Option<LoaderKind>,
    working: WorkingSample,
    upload_percent: u8,
    splash: SplashPhase,
    on_select: Option<SelectHandler>,
}

impl Loaders {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: "Loaders".into(),
            description: Some(
                "One shared clock drives every loop at about 30 fps and parks when none is on screen. Reduced motion rests each on its still frame."
                    .into(),
            ),
            groups: default_groups(),
            selected: None,
            working: WorkingSample::new("Thinking").secs(4),
            upload_percent: 64,
            splash: SplashPhase::Showing,
            on_select: None,
        }
    }

    /// The card's title (default "Loaders").
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The line under the title; `None` drops it.
    pub fn description(mut self, description: Option<SharedString>) -> Self {
        self.description = description;
        self
    }

    /// The card's rows (default [`default_groups`]).
    pub fn groups(mut self, groups: impl IntoIterator<Item = LoaderGroup>) -> Self {
        self.groups = groups.into_iter().collect();
        self
    }

    /// The preview picked at first (default: the first one).
    pub fn selected(mut self, kind: LoaderKind) -> Self {
        self.selected = Some(kind);
        self
    }

    /// What the working line reads.
    pub fn working(mut self, working: WorkingSample) -> Self {
        self.working = working;
        self
    }

    /// The upload ring's progress, 0–100.
    pub fn upload_percent(mut self, percent: u8) -> Self {
        self.upload_percent = percent.min(100);
        self
    }

    /// The boot splash's phase.
    pub fn splash(mut self, phase: SplashPhase) -> Self {
        self.splash = phase;
        self
    }

    /// Called with the loader whose preview was picked.
    pub fn on_select(
        mut self,
        handler: impl Fn(LoaderKind, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

struct LoadersState {
    seeded: bool,
    selected: LoaderKind,
    /// One tab stop per preview, in reading order.
    focus: Vec<FocusHandle>,
}

impl LoadersState {
    fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            seeded: false,
            selected: LoaderKind::BreathingOrb,
            focus: Vec::new(),
        }
    }
}

/// Where an arrow key moves the pick (a radio grid): Left and Right walk
/// reading order, Up and Down keep the column in the row above or below,
/// Home and End jump to the ends. `rows` holds each row's length.
fn step(rows: &[usize], from: usize, key: &str) -> Option<usize> {
    let total: usize = rows.iter().sum();
    if total == 0 || from >= total {
        return None;
    }
    let (row, col) = {
        let mut start = 0;
        let mut at = (0, 0);
        for (r, len) in rows.iter().enumerate() {
            if from < start + len {
                at = (r, from - start);
                break;
            }
            start += len;
        }
        at
    };
    let row_start = |r: usize| rows[..r].iter().sum::<usize>();
    let to = match key {
        "left" => from.checked_sub(1)?,
        "right" => (from + 1 < total).then_some(from + 1)?,
        "up" => {
            let r = (0..row).rev().find(|r| rows[*r] > 0)?;
            row_start(r) + col.min(rows[r] - 1)
        }
        "down" => {
            let r = (row + 1..rows.len()).find(|r| rows[*r] > 0)?;
            row_start(r) + col.min(rows[r] - 1)
        }
        "home" => 0,
        "end" => total - 1,
        _ => return None,
    };
    (to != from).then_some(to)
}

impl RenderOnce for Loaders {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            LoadersState::new,
        );
        let order: Vec<LoaderKind> = self
            .groups
            .iter()
            .flat_map(|g| g.kinds.iter().copied())
            .collect();
        let rows: Rc<[usize]> = self.groups.iter().map(|g| g.kinds.len()).collect();
        {
            let initial = self
                .selected
                .filter(|k| order.contains(k))
                .or(order.first().copied());
            let count = order.len();
            state.update(cx, |s, cx| {
                if !s.seeded {
                    s.seeded = true;
                    if let Some(kind) = initial {
                        s.selected = kind;
                    }
                }
                if s.focus.len() != count {
                    s.focus = (0..count)
                        .map(|_| cx.focus_handle().tab_stop(true))
                        .collect();
                }
            });
        }
        let selected = {
            let s = state.read(cx).selected;
            if order.contains(&s) {
                Some(s)
            } else {
                order.first().copied()
            }
        };
        let order: Rc<[LoaderKind]> = order.into();

        let live = Live {
            working: self.working.clone(),
            upload_percent: self.upload_percent,
            splash: self.splash,
        };
        let mut card = pink::section_card(&t)
            .id(self.id.clone())
            .mt_0()
            .w(px(CARD_W))
            .max_w_full()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(pink::section_header(
                self.title.clone(),
                self.description.clone(),
                &t,
            ));

        let mut ix = 0;
        for group in &self.groups {
            let mut previews = Vec::with_capacity(group.kinds.len());
            for kind in &group.kinds {
                previews.push(option_card(
                    &key,
                    ix,
                    *kind,
                    selected == Some(*kind),
                    &live,
                    Nav {
                        state: state.clone(),
                        order: order.clone(),
                        rows: rows.clone(),
                        on_select: self.on_select.clone(),
                    },
                    &t,
                    window,
                    cx,
                ));
                ix += 1;
            }
            card = card.child(
                pink::card_row(false, &t)
                    .child(pink::row_text(
                        group.title.clone(),
                        group.description.clone(),
                        &t,
                    ))
                    .child(
                        pink::row_block().child(
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .items_start()
                                .gap(px(PREVIEW_GAP))
                                .children(previews),
                        ),
                    ),
            );
        }
        card.when_some(selected, |card, kind| card.child(spec_row(&key, kind, &t)))
    }
}

/// What the live previews show, from the builder.
struct Live {
    working: WorkingSample,
    upload_percent: u8,
    splash: SplashPhase,
}

/// What a preview needs to move the pick.
struct Nav {
    state: Entity<LoadersState>,
    order: Rc<[LoaderKind]>,
    rows: Rc<[usize]>,
    on_select: Option<SelectHandler>,
}

impl Nav {
    fn pick(&self, ix: usize, focus: bool, window: &mut Window, cx: &mut App) {
        let Some(kind) = self.order.get(ix).copied() else {
            return;
        };
        let handle = self.state.update(cx, |s, cx| {
            s.selected = kind;
            cx.notify();
            s.focus.get(ix).cloned()
        });
        if focus && let Some(handle) = handle {
            window.focus(&handle, cx);
        }
        if let Some(f) = &self.on_select {
            f(kind, window, cx);
        }
    }
}

/// One preview (Pink settings/widgets.rs `option_card`): the loader framed
/// at 148 tall, rounded 6, with a 1 px `border` inside the 2 px accent
/// selection ring, and the caption under it, 8 apart – 13 in `text_muted`,
/// or 500 `text` when picked (accent stays on the ring). Under the caption,
/// where Pink shows the loader. Keyboard focus draws Pink's inset ring on
/// the frame.
#[allow(clippy::too_many_arguments)]
fn option_card(
    key: &str,
    ix: usize,
    kind: LoaderKind,
    selected: bool,
    live: &Live,
    nav: Nav,
    t: &Pink,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let handle = nav.state.read(cx).focus.get(ix).cloned();
    let focus_visible = handle
        .as_ref()
        .is_some_and(|h| h.is_focused(window) && window.last_input_was_keyboard());
    let nav = Rc::new(nav);
    let frame = div()
        .h(px(PREVIEW_H))
        .w_full()
        .rounded(px(PREVIEW_RADIUS))
        .overflow_hidden()
        .border_1()
        .border_color(t.border)
        .child(preview(key, kind, live, t, window, cx))
        .when(focus_visible, |frame| {
            // On top of the preview: the ring is a sibling laid over it.
            frame.relative().child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(PREVIEW_RADIUS - 1.))
                    .shadow(t.focus_ring_shadows()),
            )
        });
    let card = div()
        .id(SharedString::from(format!("{key}-preview-{ix}")))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(space::S8))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(SharedString::from(kind.name()))
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
        .child(pink::selection_ring(selected, PREVIEW_RADIUS, t, frame).w_full())
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(space::S2))
                .text_center()
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .font_weight(if selected {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if selected { t.text } else { t.text_muted })
                        .child(kind.name()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(t.readable_muted(0.7))
                        .child(kind.usage()),
                ),
        )
        .on_click({
            let nav = nav.clone();
            move |_, window, cx| nav.pick(ix, false, window, cx)
        })
        .on_key_down({
            let nav = nav.clone();
            move |event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if matches!(key, "enter" | "space") {
                    // The click picks it; nothing else may act on the key.
                    cx.stop_propagation();
                } else if let Some(next) = step(&nav.rows, ix, key) {
                    cx.stop_propagation();
                    nav.pick(next, true, window, cx);
                }
            }
        });
    match handle {
        Some(handle) => card.track_focus(&handle).into_any_element(),
        None => card.into_any_element(),
    }
}

/// A preview plate: the surface the loader sits on in Pink, rounded to the
/// frame (Pink: a preview rounds its own corners), its content centered.
fn plate(bg: Hsla) -> Div {
    div()
        .size_full()
        .rounded(px(PREVIEW_RADIUS - 1.))
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
}

fn preview(
    key: &str,
    kind: LoaderKind,
    live: &Live,
    t: &Pink,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match kind {
        LoaderKind::BreathingOrb => plate(t.bg).child(sending_strip(t)),
        LoaderKind::WorkingLine => plate(t.bg).child(working_line(&live.working, t)),
        LoaderKind::GlyphSpinner => plate(t.bg).child(running_tool_row(key, t)),
        LoaderKind::MonoSpinner => plate(t.chrome_surface()).child(reconnecting_line(key, t)),
        LoaderKind::GradientSpinner => plate(t.bg).child(loading_pane(t)),
        LoaderKind::SkeletonRows => plate(t.card_glass_bg()).px(px(space::S12)).child(
            div()
                .w_full()
                .child(loaders::skeleton_rows(3, t.fill(FillTier::Quaternary))),
        ),
        LoaderKind::MenuSkeleton => plate(t.surface_bg()).items_start().p(px(space::S8)).child(
            div()
                .w_full()
                .child(loaders::skeleton_menu_rows(5, t.fill(FillTier::Quaternary))),
        ),
        LoaderKind::LabelGhost => plate(t.surface_bg()).child(ghost_chip(key, t)),
        LoaderKind::UploadRing => {
            plate(t.bg).child(uploading_thumb(live.upload_percent, t, window, cx))
        }
        LoaderKind::BootSplash => {
            plate(t.bg)
                .relative()
                .when(live.splash != SplashPhase::Gone, |el| {
                    el.child(loaders::splash_overlay(
                        t.chrome_surface(),
                        t.glyph,
                        SPLASH_LABEL,
                        t.readable_muted(0.7),
                        live.splash == SplashPhase::Fading,
                    ))
                })
        }
        LoaderKind::PinkWave => plate(t.bg).child(loaders::pink_loader(6., t.text)),
        LoaderKind::PinkMark => plate(t.bg).child(loaders::pink_mark_loader(48., t.text)),
    }
    .into_any_element()
}

/// The composer's "Sending" strip (Pink shell.rs `render_status_strip`):
/// a 24 tall row, gap 8, of the breathing orb and "Sending" at 12 in
/// `text_muted`.
fn sending_strip(t: &Pink) -> Div {
    div()
        .h(px(layout::STATUS_STRIP_HEIGHT))
        .flex()
        .items_center()
        .gap(px(space::S8))
        .text_size(px(11.))
        .child(loaders::breathing_orb(t.accent))
        .child(
            div()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child("Sending"),
        )
}

/// The working line (Pink transcript.rs `render_working_trailer`): the orb
/// in its 16 line box, the status, and "12s", gap 8, top-aligned.
fn working_line(working: &WorkingSample, t: &Pink) -> Div {
    let mut status = loaders::working_status(working.verb.clone(), t.content_muted, t.text);
    if let Some(object) = &working.object {
        status = status.object(ellipsize_chars(object, LIVE_STATUS_OBJECT_CHARS));
    }
    div()
        .max_w_full()
        .px(px(space::S16))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(space::S8))
        .min_w_0()
        .child(loaders::orb_slot(t.accent))
        .child(div().min_w_0().child(status))
        .child(loaders::working_timer(working.secs, t.content_faint))
}

/// A running tool row on the activity rail (Pink transcript.rs
/// `chip_header_row`): the glyph spinner in the 14 tool-glyph slot, then the
/// label and its detail at 12/18 in `content_muted`, gap 8.
fn running_tool_row(key: &str, t: &Pink) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S8))
        .text_size(px(TOOL_LABEL_SIZE))
        .line_height(px(TOOL_LABEL_LINE_HEIGHT))
        .text_color(t.content_muted)
        .child(
            div()
                .flex_none()
                .size(px(ACTIVITY_ICON))
                .flex()
                .items_center()
                .justify_center()
                .child(loaders::mini_glyph_spinner(
                    format!("{key}-tool-spin"),
                    2.,
                    t.glyph,
                )),
        )
        .child(div().flex_none().child("Running"))
        .child(div().min_w_0().truncate().child("cargo test"))
}

/// The sidebar's connection line while reconnecting (Pink shell.rs
/// `render_connection_pill`): the mono spinner in `text_muted`, gap 6, then
/// "Reconnecting…" at 11 in `text_faint`.
fn reconnecting_line(key: &str, t: &Pink) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(space::S6))
        .child(loaders::mini_mono_spinner(
            format!("{key}-reconnect-spin"),
            2.,
            t.text_muted,
        ))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(11.))
                .text_color(t.text_faint)
                .child("Reconnecting…"),
        )
}

/// A pane still loading (Pink history.rs): the gradient matrix at cell 3
/// over "Loading history…" at 12 in `text_faint`, 8 apart.
fn loading_pane(t: &Pink) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(space::S8))
        .child(loaders::gradient_spinner(3., t.glyph))
        .child(
            div()
                .text_size(px(12.))
                .text_color(t.text_faint)
                .child("Loading history…"),
        )
}

/// A composer chip still resolving (Pink pickers.rs chip): 28 tall, round,
/// `px 10`, gap 6, the glyph spinner where the icon goes and a 56 ghost bar
/// in the Tertiary fill where the label goes.
fn ghost_chip(key: &str, t: &Pink) -> Div {
    div()
        .h(px(28.))
        .px(px(space::S10))
        .rounded_full()
        .flex()
        .items_center()
        .gap(px(space::S6))
        .child(div().flex_none().child(loaders::mini_glyph_spinner(
            format!("{key}-chip-spin"),
            2.,
            t.glyph,
        )))
        .child(loaders::skeleton_bar(
            GHOST_LABEL_W,
            t.fill(FillTier::Tertiary),
        ))
}

/// An image attachment while it uploads (Pink transcript.rs attachment
/// thumb): 112×80, rounded 8, a black dim at 38% breathing 5% more on the
/// Pink pulse, the 34 ring centered. The picture is a screenshot's worth of
/// lines on the panel tone.
fn uploading_thumb(percent: u8, t: &Pink, window: &mut Window, cx: &mut App) -> Div {
    let pulse = if motion::reduced_motion(cx) {
        0.
    } else {
        motion::pulse_wave(motion::pulse_phase(&PINK_PULSE, window, cx))
    };
    let line = |w: f32, alpha: f32| {
        div()
            .h(px(4.))
            .w(gpui::relative(w))
            .rounded(px(2.))
            .bg(t.ink(alpha))
    };
    let picture = div()
        .absolute()
        .inset_0()
        .bg(t.surface_raised)
        .px(px(space::S10))
        .py(px(space::S10))
        .flex()
        .flex_col()
        .gap(px(space::S6))
        .child(line(0.45, 0.30))
        .child(line(0.9, 0.16))
        .child(line(0.75, 0.16))
        .child(line(0.82, 0.16))
        .child(line(0.5, 0.16));
    div()
        .relative()
        .w(px(THUMB_W))
        .h(px(THUMB_H))
        .rounded(px(pink::radius::MD))
        .overflow_hidden()
        .border_1()
        .border_color(t.hairline(0.10))
        .child(picture)
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(hsla(0., 0., 0., 0.38 + 0.05 * pulse))
                .child(loaders::upload_progress_ring(percent, RING_D)),
        )
}

/// The picked loader's timing and the kit call that draws it: a settings
/// row whose copy settles in like a status label when the pick changes.
fn spec_row(key: &str, kind: LoaderKind, t: &Pink) -> Div {
    let text = pink::row_text(kind.name(), Some(kind.spec().into()), t);
    pink::card_row(false, t)
        .child(
            div()
                .flex_1()
                .min_w(px(layout::ROW_TEXT_MIN))
                .child(motion::status_swap(
                    loaders::label_id(&format!("{key}-spec"), kind.name()),
                    text,
                )),
        )
        .child(
            pink::row_controls().child(
                div()
                    .font_family(pink::MONO)
                    .text_size(px(12.))
                    .text_color(t.text_dim)
                    .child(kind.symbol()),
            ),
        )
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// The working line's steps in the demo, and how long each holds.
const STEPS: [(&str, Option<&str>); 4] = [
    ("Thinking", None),
    ("Reading", Some("ci.yml")),
    ("Running", Some("cargo test --workspace")),
    ("Editing", Some("upload.rs")),
];
const TICK: Duration = Duration::from_millis(100);
/// Ticks per working step, per splash cycle (shown, fading, gone).
const STEP_TICKS: u32 = 26;
const SPLASH_SHOWN: u32 = 30;
const SPLASH_FADING: u32 = 8;
const SPLASH_CYCLE: u32 = 46;

struct Demo {
    tick: u32,
    upload: u8,
    hold: u8,
    _task: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this
                    .update(cx, |d, cx| {
                        d.advance();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            tick: 0,
            upload: 8,
            hold: 0,
            _task: task,
        }
    }

    fn advance(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        // An upload climbs, holds a moment at 100%, then the next begins.
        if self.upload >= 100 {
            self.hold += 1;
            if self.hold > 8 {
                self.hold = 0;
                self.upload = 0;
            }
        } else {
            self.upload = (self.upload + 2).min(100);
        }
    }

    fn working(&self) -> WorkingSample {
        let (verb, object) = STEPS[(self.tick / STEP_TICKS) as usize % STEPS.len()];
        let sample = WorkingSample::new(verb).secs(4 + i64::from(self.tick / 10));
        match object {
            Some(object) => sample.object(object),
            None => sample,
        }
    }

    fn splash(&self) -> SplashPhase {
        Self::splash_at(self.tick)
    }

    fn splash_at(tick: u32) -> SplashPhase {
        match tick % SPLASH_CYCLE {
            t if t < SPLASH_SHOWN => SplashPhase::Showing,
            t if t < SPLASH_SHOWN + SPLASH_FADING => SplashPhase::Fading,
            _ => SplashPhase::Gone,
        }
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("loaders-demo", cx, |_, cx| Demo::new(cx));
        let d = demo.read(cx);
        Loaders::new("loaders")
            .working(d.working())
            .upload_percent(d.upload)
            .splash(d.splash())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_groups_show_every_loader_once() {
        let shown: Vec<LoaderKind> = default_groups().into_iter().flat_map(|g| g.kinds).collect();
        assert_eq!(shown.len(), LoaderKind::ALL.len());
        for kind in LoaderKind::ALL {
            assert_eq!(shown.iter().filter(|k| **k == kind).count(), 1, "{kind:?}");
            assert!(kind.symbol().starts_with("loaders::"));
            assert!(!kind.usage().is_empty() && !kind.spec().is_empty());
        }
    }

    #[test]
    fn arrows_walk_the_grid() {
        let rows = [4, 4, 4];
        assert_eq!(step(&rows, 0, "left"), None);
        assert_eq!(step(&rows, 0, "right"), Some(1));
        assert_eq!(step(&rows, 3, "right"), Some(4), "wraps to the next row");
        assert_eq!(step(&rows, 11, "right"), None);
        assert_eq!(step(&rows, 5, "up"), Some(1));
        assert_eq!(step(&rows, 5, "down"), Some(9));
        assert_eq!(step(&rows, 9, "down"), None);
        assert_eq!(step(&rows, 6, "home"), Some(0));
        assert_eq!(step(&rows, 6, "end"), Some(11));
        assert_eq!(step(&rows, 6, "tab"), None);
        // A shorter row keeps the nearest column.
        assert_eq!(step(&[4, 2], 3, "down"), Some(5));
        assert_eq!(step(&[4, 0, 2], 1, "down"), Some(5), "skips an empty row");
    }

    #[test]
    fn demo_splash_shows_fades_and_leaves() {
        let phases: Vec<SplashPhase> = (0..SPLASH_CYCLE).map(Demo::splash_at).collect();
        assert_eq!(phases[0], SplashPhase::Showing);
        assert!(phases.contains(&SplashPhase::Fading));
        assert_eq!(phases[SPLASH_CYCLE as usize - 1], SplashPhase::Gone);
    }
}
