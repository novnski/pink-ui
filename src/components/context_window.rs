//! Context Window – the composer's context ring and its Context menu.
//!
//! A Pink component. The chip is Pink's context-usage chip (Pink:
//! context_usage.rs `render`): a 16 px ring and the occupied share, muted,
//! warning from 75%, danger from 90% and accent while compacting. Before the
//! provider reports usage the ring is empty and faint, with no share, and
//! the chip is named "Context window: usage not reported yet" (PINK 0.30.6,
//! novnski/pink#193). Resting the pointer on it shows Pink's details card
//! (`UsageCard`) centered above the chip (Pink: popover.rs `tooltip_above`).
//!
//! The chip is a button that opens PINK's Context menu (PINK 0.31.1,
//! novnski/pink#195; Pink: pickers.rs `render_context_menu`), from the
//! chip's leading edge (popover.rs `anchored_menu_above_leading`): "Context
//! window", the usage in words, any provider note, then Compact
//! ("Compacting…" while it runs, "Compact unresolved" when its result is
//! unknown, disabled with the provider's reason when it can't) and Clear
//! context, which asks once more. ↑/↓ move over the two rows and wrap,
//! Enter acts on the highlighted one, Escape closes and disarms Clear.
//!
//! The library adds a map of the window between the usage and the controls
//! (`.window_map(false)` gives PINK's plain 280 pt menu): a field of 100
//! cells (one per percent of the window) filled column by column by the
//! instructions, tool definitions, attached files, the chat history and
//! memory, with the reply reserve as hollow cells at the end and a rule
//! where auto-compaction kicks in, and a breakdown whose rows light their
//! cells. Files can be dropped from the breakdown. Compacting waves the chat
//! history's cells on the pulse clock, then sweeps the freed cells clear.
//!
//! The ring shows only when the setting is on and the provider reports
//! context usage ([`context_ring_shown`], PINK 0.31.3, D75); without it a
//! host's model menu keeps Compact and Clear in [`context_tray`] (Pink: the
//! model menu's `context_tray`).
//!
//! ```ignore
//! ContextWindow::new("ctx", "lumen-4", 200_000)
//!     .system(3_100)
//!     .tools(14_600)
//!     .file("rules", "billing_rules.md", 18_400)
//!     .conversation(96_300, 46)
//!     .on_compact(|_, _| {})
//! ```

use std::{
    f32::consts::{FRAC_PI_2, TAU},
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnyElement, AnyView, App, Bounds, Context, Div, ElementId, Entity, FocusHandle,
    FontWeight, Hsla, KeyDownEvent, MouseDownEvent, PathBuilder, Pixels, Role, SharedString,
    Stateful, Task, Window, anchored, canvas, deferred, div, point, prelude::*, px, relative, svg,
    transparent_black,
};

use crate::{
    HoverFill,
    motion::{self, HOVER_FADE, MENU_OUT, MENU_RISE_UP, PINK_PULSE, PULSE_STAGGER, RESORT},
    pink::{
        self, FillTier, NON_TEXT_CONTRAST_TARGET, Pink, TEXT_CONTRAST_TARGET, chrome, layout,
        radius,
    },
    story,
};

pub const NAME: &str = "Context Window";
pub const DESCRIPTION: &str = "A composer's context ring and the share in use, muted, then warning and danger as it fills, with the token count on hover. It opens a Context menu with the usage in words, a map of the window one cell per percent whose breakdown lights its cells and drops files, and Compact and Clear context from the keyboard";

/// Pink: icons.rs `COLLAPSE_ARROWS`, `REFRESH`, `CLOSE`, `PAPERCLIP`, `ARROW_UP`.
const ICON_COMPACT: &str = "icons/context-window/minimize.svg";
const ICON_CLEAR: &str = "icons/context-window/refresh.svg";
const ICON_REMOVE: &str = "icons/context-window/close.svg";
const ICON_PAPERCLIP: &str = "icons/context-window/paperclip.svg";
const ICON_ARROW_UP: &str = "icons/context-window/arrow-up.svg";

/// The chip (Pink: context_usage.rs `CHIP_PAD_X`, `RING`, `RING_GAP` and the
/// ring's radius and stroke).
const CHIP_PAD_X: f32 = 6.;
const RING: f32 = 16.;
const RING_GAP: f32 = 5.;
const RING_RADIUS: f32 = 6.;
const RING_STROKE: f32 = 1.8;

/// The details card's padding and rhythm (Pink: `UsageCard::render`).
const CARD_PAD: f32 = 12.;
const CARD_GAP: f32 = 8.;

/// The field: 100 cells, so each one is a percent of the window.
const COLS: usize = 20;
const ROWS: usize = 5;
const CELL: f32 = 12.;
const CELL_GAP: f32 = 3.;
const CELL_RADIUS: f32 = 3.;
const PITCH: f32 = CELL + CELL_GAP;
const FIELD_W: f32 = COLS as f32 * PITCH - CELL_GAP;
const FIELD_H: f32 = ROWS as f32 * PITCH - CELL_GAP;
/// The card hugs the field, as Pink's details card hugs its lines.
const CARD_W: f32 = FIELD_W + 2. * CARD_PAD + 2.;
/// Breakdown rows: parts (Pink: shell/activity_center.rs alert row, 28
/// tall, gap 6), and the files under Attached files.
const ROW_H: f32 = 28.;
const ROW_GAP: f32 = 6.;
const SWATCH: f32 = 8.;
const FILE_H: f32 = 24.;
/// The tray's action rows (Pink: `render_context_controls`).
const TRAY_ROW_H: f32 = 30.;
/// Share of the window at which the conversation compacts on its own.
const AUTO_COMPACT: f32 = 0.9;
/// Chat history shorter than this has nothing to compact.
const COMPACT_MIN: u32 = 4_000;
const COMPACT_TIME: Duration = Duration::from_millis(1700);
/// Changed cells re-color one after another in fill order.
const SWEEP_STAGGER_MS: u64 = 8;
/// Unlit cells while a part is inspected.
const DIMMED: f32 = 0.22;
/// A status tone's strength across the field (Pink: shell/spaces.rs status
/// colors, `warning @ 0.72`).
const STATUS_STRENGTH: f32 = 0.72;

type IdHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// Whether a composer shows its context ring, and so its Context menu
/// (Pink: pickers.rs `Pickers::context_ring_shown`, PINK 0.31.3, D75): the
/// setting is on and the provider reports context usage. Otherwise the
/// model menu keeps Compact and Clear ([`context_tray`]) so they stay
/// reachable.
pub fn context_ring_shown(setting_on: bool, provider_reports_usage: bool) -> bool {
    setting_on && provider_reports_usage
}

/// What Compact can do right now (Pink: the provider's
/// `ContextControlCapabilities::compact` and the chat's compact status).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CompactState {
    /// The provider compacts on request.
    #[default]
    Ready,
    /// A compaction is running (`ContextOperationStatus::Running`).
    Running,
    /// The last compaction's result is unknown (`Unknown`); a new one waits
    /// until it resolves.
    Unknown,
    /// The provider can't compact (`compact_unavailable_reason`).
    Unavailable(SharedString),
}

impl CompactState {
    /// The row's label (Pink: `render_context_controls` `compact_label`).
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "Compacting…",
            Self::Unknown => "Compact unresolved",
            Self::Ready | Self::Unavailable(_) => "Compact",
        }
    }

    /// Pink: `compact.is_some() && !compact.blocks_new_turn()`.
    pub fn enabled(&self) -> bool {
        matches!(self, Self::Ready)
    }

    fn reason(&self) -> Option<SharedString> {
        match self {
            Self::Unavailable(reason) => Some(reason.clone()),
            _ => None,
        }
    }
}

/// Compact and Clear for a host's own menu when there is no ring (Pink: the
/// model menu's `context_tray`, PINK 0.31.3).
#[derive(Clone, Default)]
pub struct ContextControls {
    compact: CompactState,
    on_compact: Option<Handler>,
    on_clear: Option<Handler>,
}

impl ContextControls {
    pub fn new(compact: CompactState) -> Self {
        Self {
            compact,
            on_compact: None,
            on_clear: None,
        }
    }

    /// Compact was chosen (only while it's [`CompactState::Ready`]).
    pub fn on_compact(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_compact = Some(Rc::new(f));
        self
    }

    /// Clear context was confirmed.
    pub fn on_clear(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Rc::new(f));
        self
    }
}

/// The context tray at the foot of a model menu (Pink: pickers.rs
/// `render_context_controls` inside the model menu's `context_tray`): a
/// hairline on top, an 11 faint "Context" heading, Compact with its reason
/// when the provider can't, and Clear context, which asks once more. Debug
/// selectors: `{id}`, `{id}-compact`, `{id}-clear`.
pub fn context_tray(
    id: impl Into<SharedString>,
    controls: ContextControls,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let t = Pink::get(cx);
    let id: SharedString = id.into();
    let confirm = window.use_keyed_state(
        ElementId::NamedChild(Arc::new(eid(id.to_string())), "confirm".into()),
        cx,
        |_, _| false,
    );
    let armed = *confirm.read(cx);
    let ContextControls {
        compact: state,
        on_compact,
        on_clear,
    } = controls;
    let enabled = state.enabled();
    let compact = tray_row(
        format!("{id}-compact"),
        ICON_COMPACT,
        state.label(),
        enabled,
        false,
        true,
        &t,
    )
    .on_click(move |_, window, cx| {
        if enabled && let Some(f) = on_compact.as_ref() {
            f(window, cx);
        }
    });
    let clear = {
        let confirm = confirm.clone();
        tray_row(
            format!("{id}-clear"),
            ICON_CLEAR,
            clear_label(armed),
            true,
            false,
            true,
            &t,
        )
        .on_click(move |_, window, cx| {
            let fire = confirm.update(cx, |armed, cx| {
                *armed = !*armed;
                cx.notify();
                !*armed
            });
            if fire && let Some(f) = on_clear.as_ref() {
                f(window, cx);
            }
        })
    };
    let selector = id.to_string();
    div()
        .id(eid(id.to_string()))
        .debug_selector(move || selector)
        .flex_none()
        .border_t_1()
        .border_color(t.hairline(0.08))
        .child(
            controls_panel(&t)
                .child(caption("Context".into(), &t))
                .child(compact)
                .when_some(state.reason(), |d, reason| {
                    let selector = format!("{id}-compact-reason");
                    d.child(caption(reason, &t).debug_selector(move || selector))
                })
                .child(clear),
        )
}

/// A part of the context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    System,
    Tools,
    Files,
    Conversation,
    Memory,
}

impl Part {
    fn label(self) -> &'static str {
        match self {
            Part::System => "Instructions",
            Part::Tools => "Tool definitions",
            Part::Files => "Attached files",
            Part::Conversation => "Chat history",
            Part::Memory => "Saved memory",
        }
    }

    /// How strongly the part's cells carry the occupancy tone. Pink keeps
    /// one hue per state, so parts differ by strength, never by color; the
    /// chat history, the part compaction acts on, is full strength.
    fn strength(self) -> f32 {
        match self {
            Part::System => 0.45,
            Part::Tools => 0.65,
            Part::Files => 0.82,
            Part::Conversation => 1.,
            Part::Memory => 0.6,
        }
    }
}

/// A file attached to the context.
#[derive(Clone, Debug)]
pub struct ContextFile {
    pub id: SharedString,
    pub name: SharedString,
    pub tokens: u32,
}

/// What one cell holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Free,
    Reserved,
    /// A part, and for files the file's index.
    Part(Part, Option<usize>),
}

/// What the pointer is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    Part(Part),
    File(usize),
    Free,
    Reserved,
}

impl Spot {
    fn of(cell: Cell) -> Spot {
        match cell {
            Cell::Free => Spot::Free,
            Cell::Reserved => Spot::Reserved,
            Cell::Part(Part::Files, Some(i)) => Spot::File(i),
            Cell::Part(p, _) => Spot::Part(p),
        }
    }

    fn lights(self, cell: Cell) -> bool {
        match (self, cell) {
            (Spot::Part(p), Cell::Part(q, _)) => p == q,
            (Spot::File(i), Cell::Part(Part::Files, Some(j))) => i == j,
            (Spot::Free, Cell::Free) | (Spot::Reserved, Cell::Reserved) => true,
            _ => false,
        }
    }
}

/// Where a hover came from, so leaving one cell doesn't clear the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoverSource {
    Cell(usize),
    Row(Spot),
}

#[derive(Clone, Debug)]
struct FileEntry {
    file: ContextFile,
    removed: Option<Instant>,
}

#[derive(Clone, Debug)]
struct Usage {
    capacity: u32,
    reserve: u32,
    system: u32,
    tools: u32,
    files: Vec<FileEntry>,
    conversation: u32,
    turns: u32,
    summarized: bool,
    memory: u32,
    /// The provider has reported usage; until it does the chip shows an
    /// empty faint ring and no share (PINK 0.30.6).
    reported: bool,
}

impl Usage {
    fn live_files(&self) -> impl Iterator<Item = (usize, &FileEntry)> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.removed.is_none())
    }

    fn files_total(&self) -> u32 {
        self.live_files().map(|(_, f)| f.file.tokens).sum()
    }

    fn used(&self) -> u32 {
        self.system + self.tools + self.files_total() + self.conversation + self.memory
    }

    /// The occupied share (Pink: `ContextUsage::fraction`); can pass 1.
    fn fraction(&self) -> f32 {
        self.used() as f32 / self.capacity as f32
    }

    fn tokens(&self, spot: Spot) -> u32 {
        match spot {
            Spot::Part(Part::System) => self.system,
            Spot::Part(Part::Tools) => self.tools,
            Spot::Part(Part::Files) => self.files_total(),
            Spot::Part(Part::Conversation) => self.conversation,
            Spot::Part(Part::Memory) => self.memory,
            Spot::File(i) => self.files.get(i).map_or(0, |f| f.file.tokens),
            Spot::Free => self
                .capacity
                .saturating_sub(self.used())
                .saturating_sub(self.reserve),
            Spot::Reserved => self.reserve,
        }
    }

    /// The field, filled column by column.
    fn cells(&self) -> Vec<Cell> {
        let total = COLS * ROWS;
        let per = (self.capacity as f32 / total as f32).max(1.);
        let count = |tokens: u32| {
            if tokens == 0 {
                0
            } else {
                ((tokens as f32 / per).round() as usize).max(1)
            }
        };
        let reserved = count(self.reserve).min(total);
        let room = total - reserved;
        let mut cells = Vec::with_capacity(total);
        let push = |cell: Cell, n: usize, cells: &mut Vec<Cell>| {
            for _ in 0..n {
                if cells.len() < room {
                    cells.push(cell);
                }
            }
        };
        push(
            Cell::Part(Part::System, None),
            count(self.system),
            &mut cells,
        );
        push(Cell::Part(Part::Tools, None), count(self.tools), &mut cells);
        for (i, f) in self.live_files() {
            push(
                Cell::Part(Part::Files, Some(i)),
                count(f.file.tokens),
                &mut cells,
            );
        }
        push(
            Cell::Part(Part::Conversation, None),
            count(self.conversation),
            &mut cells,
        );
        push(
            Cell::Part(Part::Memory, None),
            count(self.memory),
            &mut cells,
        );
        cells.resize(room, Cell::Free);
        cells.resize(total, Cell::Reserved);
        cells
    }
}

/// Thousands grouped with commas (Pink: context_usage.rs `with_separators`).
fn with_separators(count: u32) -> String {
    let digits = count.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// The token lines (Pink: context_usage.rs `details`).
fn details(usage: &Usage) -> String {
    if !usage.reported {
        return "Context usage not reported by this harness yet".into();
    }
    let used = usage.used();
    format!(
        "{} of {} tokens\n{} tokens remaining",
        with_separators(used),
        with_separators(usage.capacity),
        with_separators(usage.capacity.saturating_sub(used))
    )
}

/// The state tone before flooring (Pink: context_usage.rs `tone`): accent
/// while compacting, else danger from 90%, warning from 75%, muted below.
fn tone(compacting: bool, fraction: f32, t: &Pink) -> Hsla {
    if compacting {
        t.accent
    } else if fraction >= 0.9 {
        t.danger
    } else if fraction >= 0.75 {
        t.warning
    } else {
        t.text_muted
    }
}

/// The chip's tone (Pink: context_usage.rs `tone`): with nothing reported
/// yet, `text_faint`, unless the provider is compacting.
fn chip_tone(compacting: bool, reported: bool, fraction: f32, t: &Pink) -> Hsla {
    if reported || compacting {
        tone(compacting, fraction, t)
    } else {
        t.text_faint
    }
}

/// The chip's accessible name (Pink: context_usage.rs `render`, PINK
/// 0.30.6): "Context window: 79% used", "Context window: compacting", and
/// before the provider reports anything "Context window: usage not reported
/// yet" rather than a dash placeholder.
fn accessible_name(compacting: bool, reported: bool, text: &str) -> SharedString {
    if compacting {
        "Context window: compacting".into()
    } else if !reported {
        "Context window: usage not reported yet".into()
    } else {
        format!("Context window: {text} used").into()
    }
}

/// The chip's text (Pink: context_usage.rs `label`).
fn label(compacting: bool, fraction: f32) -> String {
    if compacting {
        "Compacting…".into()
    } else {
        format!("{:.0}%", fraction * 100.)
    }
}

struct WindowState {
    usage: Usage,
    hover: Option<(Spot, HoverSource)>,
    focus: FocusHandle,
    open: bool,
    /// Bumped on every open so the entrance replays.
    opened: u32,
    closing: Option<Instant>,
    trigger: Bounds<Pixels>,
    /// Pink's details card, shown once the pointer has rested on the chip
    /// for `TOOLTIP_DELAY`.
    peek: bool,
    /// A compaction started from the tray.
    compacting: Option<Instant>,
    /// What the host says Compact can do; `Running` holds the chip on
    /// Compacting… (Pink: the chat's compact status and the provider's
    /// capability).
    compact: CompactState,
    /// The Context menu row the arrows rest on (Pink: `active`, none until
    /// the first arrow).
    active: Option<usize>,
    /// The open menu's keyboard focus.
    menu_focus: FocusHandle,
    /// Clear context asks once more before it acts (Pink:
    /// `clear_context_confirmation`).
    confirm_clear: bool,
    /// What the last context operation did (Pink: `compact.detail`).
    note: Option<SharedString>,
    /// The field before the last change, for the sweep.
    prev: Vec<Cell>,
    changed: Option<Instant>,
    _task: Option<Task<()>>,
    _close_task: Option<Task<()>>,
    _peek_task: Option<Task<()>>,
}

impl WindowState {
    fn hover(&mut self, spot: Spot, source: HoverSource, hovered: bool) {
        if hovered {
            self.hover = Some((spot, source));
        } else if self.hover.is_some_and(|(_, s)| s == source) {
            self.hover = None;
        }
    }

    fn is_compacting(&self) -> bool {
        self.compacting.is_some() || self.compact == CompactState::Running
    }

    /// Compact's row label: the demo's own compaction reads Compacting…,
    /// else the host's state does (Pink: `compact_label`).
    fn compact_label(&self) -> &'static str {
        if self.compacting.is_some() {
            CompactState::Running.label()
        } else {
            self.compact.label()
        }
    }

    /// The details card's operation line (Pink: `compact.detail`, else the
    /// idle sentence).
    fn detail(&self) -> SharedString {
        match &self.note {
            Some(note) => note.clone(),
            None if self.compact == CompactState::Running => {
                format!("Compacting {} turns into a summary…", self.usage.turns).into()
            }
            None => "No provider context operation is pending.".into(),
        }
    }

    /// The pointer arrived on or left the chip: Pink's tooltip delay, then
    /// the details card; leaving hides it at once.
    fn hover_chip(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if !hovered {
            self._peek_task = None;
            if self.peek {
                self.peek = false;
                cx.notify();
            }
            return;
        }
        if self.open || self.peek {
            return;
        }
        self._peek_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(layout::TOOLTIP_DELAY).await;
            let _ = this.update(cx, |s, cx| {
                if !s.open {
                    s.peek = true;
                    cx.notify();
                }
            });
        }));
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.peek = false;
        self._peek_task = None;
        if self.open {
            self.close_card(true, window, cx);
        } else {
            self.open = true;
            self.opened += 1;
            self.active = None;
            self.closing = None;
            self._close_task = None;
            window.focus(&self.menu_focus, cx);
            cx.notify();
        }
    }

    /// ↑/↓ over Compact and Clear, wrapping (Pink: `popover::menu_step`
    /// over `PickerKind::Context`'s two rows).
    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        self.active = Some(match self.active {
            None if delta >= 0 => 0,
            None => 1,
            Some(at) => (at as isize + delta).rem_euclid(2) as usize,
        });
        cx.notify();
    }

    /// Starts the exit fade; the card unmounts when it ends.
    fn close_card(&mut self, refocus: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.hover = None;
        self.active = None;
        self.confirm_clear = false;
        self.closing = Some(Instant::now());
        let wait = MENU_OUT.total().mul_f32(motion::speed_scale()) + Duration::from_millis(16);
        self._close_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |s, cx| {
                s.closing = None;
                cx.notify();
            });
        }));
        if refocus {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn mark_change(&mut self) {
        self.prev = self.usage.cells();
        self.changed = Some(Instant::now());
    }

    fn remove_file(&mut self, ix: usize) -> Option<SharedString> {
        let entry = self.usage.files.get(ix)?;
        if entry.removed.is_some() {
            return None;
        }
        self.mark_change();
        self.usage.files[ix].removed = Some(Instant::now());
        if matches!(self.hover, Some((Spot::File(i), _)) if i == ix) {
            self.hover = None;
        }
        Some(self.usage.files[ix].file.id.clone())
    }

    fn can_compact(&self) -> bool {
        self.compact.enabled() && !self.is_compacting() && self.usage.conversation > COMPACT_MIN
    }

    fn start_compaction(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.can_compact() {
            return false;
        }
        self.compacting = Some(Instant::now());
        self.hover = None;
        self.confirm_clear = false;
        self.note = Some(format!("Compacting {} turns into a summary…", self.usage.turns).into());
        self._task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(COMPACT_TIME.mul_f32(motion::speed_scale()))
                .await;
            this.update(cx, |s, cx| {
                s.finish_compaction();
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
        true
    }

    fn finish_compaction(&mut self) {
        self.mark_change();
        let summary = ((self.usage.conversation as f32 * 0.08) as u32)
            .max(1_800)
            .min(self.usage.conversation);
        self.note = Some(
            format!(
                "Compacted {} turns into a {}-token summary.",
                self.usage.turns,
                with_separators(summary)
            )
            .into(),
        );
        self.usage.conversation = summary;
        self.usage.summarized = true;
        self.compacting = None;
    }

    /// Pink: the tray's Clear context, which asks to be confirmed first.
    /// Returns whether the history was cleared.
    fn clear(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.confirm_clear {
            self.confirm_clear = true;
            cx.notify();
            return false;
        }
        self.confirm_clear = false;
        self.mark_change();
        self.compacting = None;
        self._task = None;
        self.note = Some(match self.usage.turns {
            0 => "Cleared the chat history.".into(),
            1 => "Cleared 1 turn of chat history.".into(),
            n => format!("Cleared {n} turns of chat history.").into(),
        });
        self.usage.conversation = 0;
        self.usage.turns = 0;
        self.usage.summarized = false;
        cx.notify();
        true
    }
}

/// The callbacks, shared by the card's listeners.
struct Props {
    /// A note from the provider, shown under the usage until an operation
    /// here leaves its own (Pink: `compact.detail`, else `notice`).
    note: Option<SharedString>,
    on_remove_file: Option<IdHandler>,
    on_compact: Option<Handler>,
    on_clear: Option<Handler>,
}

/// A model's context window: the composer chip and its Context menu.
#[derive(IntoElement)]
pub struct ContextWindow {
    id: ElementId,
    usage: Usage,
    show_label: bool,
    default_open: bool,
    compact: CompactState,
    window_map: bool,
    note: Option<SharedString>,
    on_remove_file: Option<IdHandler>,
    on_compact: Option<Handler>,
    on_clear: Option<Handler>,
}

impl ContextWindow {
    /// The model's name is kept for the API's sake: the menu reads "Context
    /// window", as PINK's does, rather than naming the model.
    pub fn new(id: impl Into<ElementId>, _model: impl Into<SharedString>, capacity: u32) -> Self {
        Self {
            id: id.into(),
            usage: Usage {
                capacity: capacity.max(1),
                reserve: 0,
                system: 0,
                tools: 0,
                files: Vec::new(),
                conversation: 0,
                turns: 0,
                summarized: false,
                memory: 0,
                reported: true,
            },
            show_label: true,
            default_open: false,
            compact: CompactState::Ready,
            window_map: true,
            note: None,
            on_remove_file: None,
            on_compact: None,
            on_clear: None,
        }
    }

    pub fn system(mut self, tokens: u32) -> Self {
        self.usage.system = tokens;
        self
    }

    pub fn tools(mut self, tokens: u32) -> Self {
        self.usage.tools = tokens;
        self
    }

    pub fn file(
        mut self,
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        tokens: u32,
    ) -> Self {
        self.usage.files.push(FileEntry {
            file: ContextFile {
                id: id.into(),
                name: name.into(),
                tokens,
            },
            removed: None,
        });
        self
    }

    /// The conversation so far and how many turns it holds.
    pub fn conversation(mut self, tokens: u32, turns: u32) -> Self {
        self.usage.conversation = tokens;
        self.usage.turns = turns;
        self
    }

    pub fn memory(mut self, tokens: u32) -> Self {
        self.usage.memory = tokens;
        self
    }

    /// Tokens kept free for the model's reply.
    pub fn reserve(mut self, tokens: u32) -> Self {
        self.usage.reserve = tokens;
        self
    }

    /// Whether the chip shows the share beside its ring (Pink gives it up
    /// when the composer row is tight; the card says it either way).
    /// The provider hasn't reported usage yet: an empty faint ring, no
    /// share, and the name "Context window: usage not reported yet" (PINK
    /// 0.30.6, novnski/pink#193).
    pub fn unreported(mut self, unreported: bool) -> Self {
        self.usage.reported = !unreported;
        self
    }

    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    /// Starts with the Context menu open.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// The provider is compacting this chat (Pink: compact status
    /// `Running`): the chip reads Compacting… in the accent, the chat
    /// history's cells wave and Compact waits, for as long as this is set.
    /// Shorthand for `.compact(CompactState::Running)`.
    pub fn compacting(mut self, running: bool) -> Self {
        if running {
            self.compact = CompactState::Running;
        } else if self.compact == CompactState::Running {
            self.compact = CompactState::Ready;
        }
        self
    }

    /// What Compact can do: ready, running, unresolved or unavailable with
    /// the provider's reason (PINK 0.31.1).
    pub fn compact(mut self, compact: CompactState) -> Self {
        self.compact = compact;
        self
    }

    /// Whether the menu maps the window cell by cell between the usage and
    /// the controls (the library's addition, on by default). Off, the menu
    /// is PINK's plain 280 pt Context menu.
    pub fn window_map(mut self, map: bool) -> Self {
        self.window_map = map;
        self
    }

    /// A provider note under the usage (Pink: the chat's `compact.detail`
    /// or context `notice`).
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// A file was dropped from the context (it leaves the field right away).
    pub fn on_remove_file(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove_file = Some(Rc::new(f));
        self
    }

    /// Compaction started.
    pub fn on_compact(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_compact = Some(Rc::new(f));
        self
    }

    /// The chat history was cleared (after its confirmation).
    pub fn on_clear(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_clear = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ContextWindow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial = self.usage.clone();
        let default_open = self.default_open;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |_, cx| WindowState {
                prev: initial.cells(),
                usage: initial,
                hover: None,
                focus: cx.focus_handle().tab_stop(true),
                open: default_open,
                opened: u32::from(default_open),
                closing: None,
                trigger: Bounds::default(),
                peek: false,
                compacting: None,
                compact: CompactState::Ready,
                active: None,
                menu_focus: cx.focus_handle(),
                confirm_clear: false,
                note: None,
                changed: None,
                _task: None,
                _close_task: None,
                _peek_task: None,
            },
        );
        if state.read(cx).compact != self.compact {
            let compact = self.compact.clone();
            state.update(cx, |s, _| s.compact = compact);
        }
        let window_map = self.window_map;
        let props = Rc::new(Props {
            note: self.note,
            on_remove_file: self.on_remove_file,
            on_compact: self.on_compact,
            on_clear: self.on_clear,
        });

        // The compaction wave runs on the shared pulse clock while the card
        // shows it.
        let (compacting, shown) = {
            let s = state.read(cx);
            (s.is_compacting(), s.open || s.closing.is_some())
        };
        let wave = (compacting && shown && !motion::reduced_motion(cx))
            .then(|| motion::pulse_phase(&PINK_PULSE, window, cx));

        let s = state.read(cx);
        let fraction = if s.usage.reported {
            s.usage.fraction()
        } else {
            0.
        };
        let tone = chip_tone(compacting, s.usage.reported, fraction, &t);
        let chip = chip(&key, s, fraction, tone, self.show_label, &t, &state);
        // Pink: popover.rs `tooltip_above`, the details card centered 7 px
        // above the chip.
        let peek = (s.peek && !s.open && s.closing.is_none()).then(|| {
            above(
                div()
                    .pb(px(layout::TOOLTIP_ABOVE_GAP))
                    .child(usage_card(s, &t)),
            )
        });
        let layer = (s.open || s.closing.is_some()).then(|| {
            let exit = (!s.open)
                .then(|| s.closing.map(motion::menu_exit_progress))
                .flatten();
            let inner = div()
                .occlude()
                .pb(px(layout::POPOVER_GAP))
                .child(window_card(
                    &key, s, tone, wave, window_map, &props, &state, window, &t, cx,
                ));
            let inner: AnyElement = match exit {
                // A dying card takes no clicks (Pink: `menu_motion_from`).
                Some(progress) => motion::menu_out(
                    ElementId::NamedInteger("card-out".into(), u64::from(s.opened)),
                    progress,
                    inner.relative().child(div().absolute().inset_0().occlude()),
                )
                .into_any_element(),
                None => motion::menu_in_from(
                    ElementId::NamedInteger("card-in".into(), u64::from(s.opened)),
                    MENU_RISE_UP,
                    inner,
                )
                .into_any_element(),
            };
            // From the chip's leading edge (Pink: `anchored_menu_above_leading`).
            above_leading(inner)
        });

        // Pink: `Pickers::on_key_down` for `PickerKind::Context`. Enter acts
        // only from the menu's own focus: a focused chip already takes Enter
        // as a click.
        let on_key = {
            let state = state.clone();
            let props = props.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let (open, active, in_menu) = {
                    let s = state.read(cx);
                    (s.open, s.active, s.menu_focus.is_focused(window))
                };
                if !open {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" => state.update(cx, |s, cx| s.close_card(true, window, cx)),
                    "up" => state.update(cx, |s, cx| s.step(-1, cx)),
                    "down" => state.update(cx, |s, cx| s.step(1, cx)),
                    "enter" if in_menu => {
                        if let Some(row) = active {
                            activate(&state, &props, row, window, cx);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }
        };

        let el = div()
            .id(self.id)
            .relative()
            .flex_none()
            .font_family(pink::SANS)
            .on_key_down(on_key)
            .child(chip)
            .children(peek)
            .children(layer);
        motion::keep_alive(window);
        el
    }
}

/// Floats `card` above the chip from its leading edge, shifted left by the
/// card inset so the rows' text lines up with the ring, kept inside the
/// window (Pink: popover.rs `anchored_menu_above_leading`).
fn above_leading(card: impl IntoElement) -> Div {
    div().absolute().top_0().left_0().size_0().child(
        deferred(
            anchored()
                .anchor(Anchor::BottomLeft)
                .offset(point(px(-layout::CARD_INSET), px(0.)))
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(card),
        )
        .with_priority(1),
    )
}

/// Floats `card` centered above the chip, kept inside the window (Pink:
/// popover.rs `AboveTooltip`, `snap_to_window_with_margin(8)`).
fn above(card: impl IntoElement) -> Div {
    div().absolute().top_0().left(relative(0.5)).size_0().child(
        deferred(
            anchored()
                .anchor(Anchor::BottomCenter)
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(card),
        )
        .with_priority(1),
    )
}

/// The context chip (Pink: context_usage.rs `render`): the ring, and the
/// share in tabular digits, both in the state tone floored on the composer
/// plate; the `element_hover` wash under the pointer, and here also while
/// its card is open. Resting on it shows Pink's details card.
fn chip(
    key: &str,
    s: &WindowState,
    fraction: f32,
    tone: Hsla,
    show_label: bool,
    t: &Pink,
    state: &Entity<WindowState>,
) -> Stateful<Div> {
    let compacting = s.is_compacting();
    let color = t.readable_on_composer(tone, TEXT_CONTRAST_TARGET);
    let ring_color = t.readable_on_composer(tone, NON_TEXT_CONTRAST_TARGET);
    let track = t.text_faint.opacity(0.25);
    let text = label(compacting, fraction);
    let reported = s.usage.reported;
    let accessible = accessible_name(compacting, reported, &text);
    let hover_key = format!("{key}-chip");
    let ring = t.focus_ring();
    let toggler = state.clone();
    let measurer = state.clone();
    let peeker = state.clone();
    let fade = motion::hover_listener(hover_key.clone());
    let rest = if s.open {
        t.element_hover
    } else {
        transparent_black()
    };
    let selector = hover_key.clone();
    div()
        .id(eid(hover_key.clone()))
        .debug_selector(move || selector)
        .role(Role::Button)
        .aria_label(accessible)
        .aria_expanded(s.open)
        .track_focus(&s.focus)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(RING_GAP))
        .h(px(layout::COMPOSER_CONTROL))
        .px(px(CHIP_PAD_X))
        .rounded_full()
        .text_size(px(12.))
        .text_color(color)
        .cursor_pointer()
        .bg(motion::hover_blend(&hover_key, rest, t.element_hover))
        .on_hover(move |hovered, window, cx| {
            fade(hovered, window, cx);
            peeker.update(cx, |s, cx| s.hover_chip(*hovered, cx));
        })
        .on_click(move |_, window, cx| toggler.update(cx, |s, cx| s.toggle(window, cx)))
        .child(occupancy_ring(fraction, ring_color, track))
        // Nothing reported yet: the ring alone, never a dash placeholder.
        .when(show_label && (reported || compacting), |el| {
            el.font_features(pink::tabular_figures()).child(text)
        })
        .child(
            canvas(
                move |bounds, _, cx| measurer.update(cx, |s, _| s.trigger = bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
}

/// The chip's 16 px ring (Pink: context_usage.rs `render`'s canvas): the
/// full track, then the occupied arc from twelve o'clock, radius 6 at a
/// 1.8 px stroke.
fn occupancy_ring(fraction: f32, color: Hsla, track: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let center = bounds.center();
            let mut arc = |fraction: f32, color: Hsla| {
                if fraction <= 0. {
                    return;
                }
                let steps = (64. * fraction).ceil().max(2.) as usize;
                let mut path = PathBuilder::stroke(px(RING_STROKE));
                for i in 0..=steps {
                    let angle = -FRAC_PI_2 + TAU * fraction * i as f32 / steps as f32;
                    let p = point(
                        center.x + px(RING_RADIUS * angle.cos()),
                        center.y + px(RING_RADIUS * angle.sin()),
                    );
                    if i == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            };
            arc(1., track);
            arc(fraction.clamp(0., 1.), color);
        },
    )
    .flex_none()
    .size(px(RING))
}

/// Pink: context_usage.rs `UsageCard`, the chip's hover card: "Context
/// window", the token lines and the operation note, on the tooltip plate.
fn usage_card(s: &WindowState, t: &Pink) -> Div {
    pink::popover_card(t)
        .bg(t.tooltip_surface())
        .p(px(CARD_PAD))
        .flex()
        .flex_col()
        .gap(px(CARD_GAP))
        .child(card_title(t))
        .child(card_details(&s.usage, t))
        .child(
            card_note(
                format!(
                    "{}\nContext pressure is separate from account quota, rate limits, and cost.",
                    s.detail()
                ),
                t,
            )
            .whitespace_nowrap(),
        )
}

/// Pink: `UsageCard`'s 12 medium title.
fn card_title(t: &Pink) -> Div {
    div()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text)
        .child("Context window")
}

/// Pink: `UsageCard`'s token lines, 12/19 muted, broken only at their own
/// newline.
fn card_details(usage: &Usage, t: &Pink) -> Div {
    div()
        .text_size(px(12.))
        .line_height(px(19.))
        .whitespace_nowrap()
        .text_color(t.text_muted)
        .child(SharedString::from(details(usage)))
}

/// Pink: `UsageCard`'s note, 11/17 faint.
fn card_note(note: String, t: &Pink) -> Div {
    div()
        .text_size(px(11.))
        .line_height(px(17.))
        .text_color(t.text_faint)
        .child(SharedString::from(note))
}

/// The Context menu (Pink: pickers.rs `render_context_menu`, PINK 0.31.1):
/// on the flush popover plate, "Context window" with the usage in words and
/// any provider note, then Compact and Clear. With the window map (the
/// library's addition) the field, its breakdown and their caption sit
/// between the two, above a hairline; without it the card is PINK's 280 pt
/// `MENU_W_M`. It holds the keyboard focus while open.
#[allow(clippy::too_many_arguments)]
fn window_card(
    key: &str,
    s: &WindowState,
    tone: Hsla,
    wave: Option<f32>,
    window_map: bool,
    props: &Rc<Props>,
    state: &Entity<WindowState>,
    window: &mut Window,
    t: &Pink,
    cx: &App,
) -> Div {
    let hover = s.hover.map(|(spot, _)| spot);
    // The cells carry the ring's tone, floored on the card. Across a whole
    // field a status tone runs below full strength, as Pink's sidebar status
    // colors do (Pink: shell/spaces.rs, warning @ 0.72).
    let strength = if tone == t.text_muted {
        1.
    } else {
        STATUS_STRENGTH
    };
    let tone = t
        .readable_on(tone, t.surface_bg(), NON_TEXT_CONTRAST_TARGET)
        .opacity(strength);
    // Pink: `compact.detail`, else the context `notice`; nothing when the
    // provider has nothing to say.
    let note: Option<SharedString> = if s.note.is_some() || s.compact == CompactState::Running {
        Some(s.detail())
    } else {
        props.note.clone()
    };
    let closer = state.clone();
    let selector = format!("{key}-menu");
    pink::popover_card_flush(t)
        .debug_selector(move || selector)
        .track_focus(&s.menu_focus)
        .w(px(if window_map { CARD_W } else { layout::MENU_W_M }))
        .flex()
        .flex_col()
        .gap_0()
        .on_mouse_down_out(move |event: &MouseDownEvent, window, cx| {
            closer.update(cx, |s, cx| {
                if s.open && !s.trigger.contains(&event.position) {
                    s.close_card(false, window, cx);
                }
            })
        })
        .child(menu_header(&s.usage, note, t))
        .when(window_map, |card| {
            card.child(
                div()
                    .px(px(CARD_PAD))
                    .pt(px(CARD_GAP))
                    .pb(px(CARD_PAD))
                    .flex()
                    .flex_col()
                    .gap(px(CARD_GAP))
                    .child(field(key, s, tone, hover, wave, state, window, t, cx))
                    .child(breakdown(key, s, tone, hover, props, state, t))
                    .child(card_note(
                        format!(
                            "Compacts on its own at the marker, {:.0}% full.\nEach cell is 1% of the window.",
                            AUTO_COMPACT * 100.
                        ),
                        t,
                    )),
            )
        })
        .child(
            tray(key, s, props, state, t)
                .when(window_map, |d| d.border_t_1().border_color(t.hairline(0.08))),
        )
}

/// The Context menu's header (Pink: `render_context_menu` `header`): inset
/// by the card and 8 more, 10 from the top, 4 apart: "Context window" (12
/// medium), the usage in words (12/18 muted, Pink: context_usage.rs
/// `details`) and the provider's note (11/16 faint).
fn menu_header(usage: &Usage, note: Option<SharedString>, t: &Pink) -> Div {
    div().px(px(layout::CARD_INSET)).child(
        div()
            .px(px(8.))
            .pt(px(10.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child("Context window"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(t.text_muted)
                    .child(SharedString::from(details(usage))),
            )
            .when_some(note, |header, note| {
                header.child(
                    div()
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .text_color(t.text_faint)
                        .child(note),
                )
            }),
    )
}

/// The color a cell rests at: parts in the tone at their strength, free
/// cells on the ring's track.
fn cell_color(cell: Cell, tone: Hsla, t: &Pink) -> Hsla {
    match cell {
        Cell::Free => t.text_faint.opacity(0.25),
        Cell::Reserved => t.text_faint.opacity(0.),
        Cell::Part(p, _) => tone.opacity(p.strength()),
    }
}

/// The reserve's hollow cells and swatch (Pink's dashed "missing" frame,
/// `hairline(0.14)`, drawn solid at this size).
fn reserve_edge(t: &Pink) -> Hsla {
    t.hairline(0.14)
}

/// The field: one cell per percent of the window, filled column by column.
/// Hovering a cell lights its part and dims the rest; changes sweep through
/// the changed cells in fill order; while compacting, the chat history's
/// cells wave on the pulse clock.
#[allow(clippy::too_many_arguments)]
fn field(
    key: &str,
    s: &WindowState,
    tone: Hsla,
    hover: Option<Spot>,
    wave: Option<f32>,
    state: &Entity<WindowState>,
    window: &mut Window,
    t: &Pink,
    cx: &App,
) -> impl IntoElement {
    let cells = s.usage.cells();
    let reduced = motion::reduced_motion(cx);
    let elapsed = s.changed.map(|c| c.elapsed());
    let edge = reserve_edge(t);
    let mut rank = 0u64;
    let mut sweeping = false;

    let children: Vec<_> = cells
        .iter()
        .enumerate()
        .map(|(i, &cell)| {
            let (col, row) = (i / ROWS, i % ROWS);
            let mut color = cell_color(cell, tone, t);
            let before = s.prev.get(i).copied().unwrap_or(cell);
            if before != cell
                && let Some(elapsed) = elapsed
                && !reduced
            {
                let spec = RESORT.with_delay(rank * SWEEP_STAGGER_MS);
                rank += 1;
                if elapsed < spec.total().mul_f32(motion::speed_scale()) {
                    sweeping = true;
                    color = motion::mix(
                        cell_color(before, tone, t),
                        color,
                        spec.progress_at(elapsed),
                    );
                }
            }
            if let (Some(phase), Cell::Part(Part::Conversation, _)) = (wave, cell) {
                let crest = motion::pulse_wave(motion::staggered_phase(phase, col, PULSE_STAGGER));
                color = color.opacity(0.35 + 0.65 * crest);
            }
            let lit = hover.is_none_or(|spot| spot.lights(cell));
            let opacity = motion::follow(
                &format!("{key}-cell-{i}"),
                if lit { 1. } else { DIMMED },
                HOVER_FADE,
            );
            let spot = Spot::of(cell);
            let hover_state = state.clone();
            div()
                .id(eid(format!("{key}-cell-{i}")))
                .absolute()
                .left(px(col as f32 * PITCH))
                .top(px(row as f32 * PITCH))
                .size(px(CELL))
                .rounded(px(CELL_RADIUS))
                .opacity(opacity)
                .map(|d| {
                    if cell == Cell::Reserved {
                        d.border_1().border_color(edge)
                    } else {
                        d.bg(color)
                    }
                })
                .on_hover(move |hovered, _, cx| {
                    hover_state.update(cx, |s, cx| {
                        s.hover(spot, HoverSource::Cell(i), *hovered);
                        cx.notify();
                    })
                })
        })
        .collect();
    if sweeping {
        window.request_animation_frame();
    }

    // Auto-compaction's rule, in the gap before its column.
    let marker_col = (COLS as f32 * AUTO_COMPACT).round();
    let marker = div()
        .absolute()
        .left(px(marker_col * PITCH - CELL_GAP + 1.))
        .top(px(-4.))
        .w(px(1.))
        .h(px(FIELD_H + 8.))
        .bg(t.border_strong);

    div()
        .flex_none()
        .relative()
        .w(px(FIELD_W))
        .h(px(FIELD_H))
        .children(children)
        .child(marker)
}

/// A breakdown row's swatch: a small cell in the part's color, or the
/// reserve's hollow cell.
fn swatch(color: Option<Hsla>, t: &Pink) -> Div {
    div()
        .flex_none()
        .size(px(SWATCH))
        .rounded(px(2.))
        .map(|d| match color {
            Some(c) => d.bg(c),
            None => d.border_1().border_color(reserve_edge(t)),
        })
}

/// The breakdown under the field. Rows follow Pink's quiet list rows (the
/// activity center's alert row: 12 label at `text @ 0.85`, 11 meta, the
/// Tertiary wash on hover) and light their cells; a lit cell washes its
/// row back. Counts are tabular so they line up.
fn breakdown(
    key: &str,
    s: &WindowState,
    tone: Hsla,
    hover: Option<Spot>,
    props: &Rc<Props>,
    state: &Entity<WindowState>,
    t: &Pink,
) -> Div {
    let usage = &s.usage;
    let wash = t.fill(FillTier::Tertiary);
    let row = |spot: Spot, color: Option<Hsla>, label: &'static str, meta: Option<String>| {
        let row_key = format!("{key}-row-{spot:?}");
        let lift = if hover == Some(spot) {
            1.
        } else {
            motion::hover_t(&row_key)
        };
        let hover_state = state.clone();
        let fade = motion::hover_listener(row_key.clone());
        div()
            .id(eid(row_key))
            .h(px(ROW_H))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .rounded(px(chrome::RADIUS_M))
            .bg(motion::mix(wash.opacity(0.), wash, lift))
            .on_hover(move |hovered, window, cx| {
                fade(hovered, window, cx);
                hover_state.update(cx, |s, cx| {
                    s.hover(spot, HoverSource::Row(spot), *hovered);
                    cx.notify();
                })
            })
            .child(swatch(color, t))
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(motion::mix(t.text.opacity(0.85), t.text, lift))
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.))
                    .text_color(t.readable_muted(0.55))
                    .child(meta.unwrap_or_default()),
            )
            .child(count(usage.tokens(spot), t.text_muted))
    };
    let part = |p: Part| Some(tone.opacity(p.strength()));

    let live = usage.live_files().count();
    let files = usage
        .files
        .iter()
        .enumerate()
        .filter_map(|(i, entry)| file_row(key, i, entry, hover, wash, props, state, t));

    div()
        .mx(px(-8.))
        .flex()
        .flex_col()
        .when(usage.system > 0, |d| {
            d.child(row(
                Spot::Part(Part::System),
                part(Part::System),
                Part::System.label(),
                None,
            ))
        })
        .when(usage.tools > 0, |d| {
            d.child(row(
                Spot::Part(Part::Tools),
                part(Part::Tools),
                Part::Tools.label(),
                None,
            ))
        })
        .child(row(
            Spot::Part(Part::Files),
            part(Part::Files),
            Part::Files.label(),
            Some(match live {
                0 => "none".into(),
                1 => "1 file".into(),
                n => format!("{n} files"),
            }),
        ))
        .children(files)
        .child(row(
            Spot::Part(Part::Conversation),
            part(Part::Conversation),
            Part::Conversation.label(),
            Some(match (usage.turns, usage.summarized) {
                (0, _) => "empty".into(),
                (n, true) => format!("summary of {n} turns"),
                (1, false) => "1 turn".into(),
                (n, false) => format!("{n} turns"),
            }),
        ))
        .when(usage.memory > 0, |d| {
            d.child(row(
                Spot::Part(Part::Memory),
                part(Part::Memory),
                Part::Memory.label(),
                None,
            ))
        })
        .when(usage.reserve > 0, |d| {
            d.child(row(Spot::Reserved, None, "Reply reserve", None))
        })
}

/// A token count, grouped and tabular.
fn count(tokens: u32, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(px(12.))
        .font_features(pink::tabular_figures())
        .text_color(color)
        .child(with_separators(tokens))
}

/// An attached file under Attached files: its name in mono (Pink keeps mono
/// for paths) aligned with the part labels, and its count, which gives way
/// on hover to Pink's row action (Pink: shell.rs sidebar row actions, 20 px
/// at radius 4 with a 12 px glyph) to drop the file. A dropped file folds
/// away.
#[allow(clippy::too_many_arguments)]
fn file_row(
    key: &str,
    ix: usize,
    entry: &FileEntry,
    hover: Option<Spot>,
    wash: Hsla,
    props: &Rc<Props>,
    state: &Entity<WindowState>,
    t: &Pink,
) -> Option<Stateful<Div>> {
    let file_key = format!("{key}-file-{}", entry.file.id);
    let height = match entry.removed {
        Some(at)
            if at.elapsed()
                > motion::DISCLOSURE
                    .total()
                    .mul_f32(motion::speed_scale() * 1.5) =>
        {
            return None;
        }
        Some(_) => motion::follow_from(&format!("{file_key}-h"), FILE_H, 0., motion::DISCLOSURE),
        None => FILE_H,
    };
    let spot = Spot::File(ix);
    let lift = if hover == Some(spot) {
        1.
    } else {
        motion::hover_t(&file_key)
    };
    let group = SharedString::from(format!("{file_key}-group"));
    let hover_state = state.clone();
    let remove_state = state.clone();
    let props = props.clone();
    let ring = t.focus_ring();
    let name = entry.file.name.clone();
    // Keyboard focus shows the button on its own plate, over the count.
    let plate = t.surface_bg();
    let remove = div()
        .id(eid(format!("{file_key}-remove")))
        .role(Role::Button)
        .aria_label(SharedString::from(format!("Remove {name}")))
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring).opacity(1.).bg(plate))
        .flex_none()
        .size(px(chrome::MIN_HIT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_S))
        .cursor_pointer()
        .opacity(0.)
        .group_hover(group.clone(), |s| s.opacity(1.))
        .hover_wash(format!("{file_key}-remove"), t.fill(FillTier::Tertiary))
        .child(
            svg()
                .path(ICON_REMOVE)
                .size(px(chrome::ICON_S))
                .text_color(t.text_muted),
        )
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            let removed = remove_state.update(cx, |s, cx| {
                let id = s.remove_file(ix);
                cx.notify();
                id
            });
            if let (Some(id), Some(f)) = (removed, props.on_remove_file.as_ref()) {
                f(&id, window, cx);
            }
        });
    Some(
        div()
            .id(eid(file_key.clone()))
            .group(group.clone())
            .flex_none()
            .h(px(height))
            .opacity(height / FILE_H)
            .overflow_hidden()
            // Names start on the part labels' column.
            .pl(px(8. + SWATCH + ROW_GAP))
            .pr(px(8.))
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .rounded(px(chrome::RADIUS_M))
            .bg(motion::mix(wash.opacity(0.), wash, lift))
            .when(entry.removed.is_none(), |d| {
                let fade = motion::hover_listener(file_key.clone());
                d.on_hover(move |hovered, window, cx| {
                    fade(hovered, window, cx);
                    hover_state.update(cx, |s, cx| {
                        s.hover(spot, HoverSource::Row(spot), *hovered);
                        cx.notify();
                    })
                })
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(pink::MONO)
                    .text_size(px(11.))
                    .text_color(motion::mix(t.readable_muted(0.85), t.text, lift))
                    .child(name.clone()),
            )
            .child(
                // Pink: shell.rs sidebar corner, where the time gives way to
                // the row's actions on hover.
                div()
                    .flex_none()
                    .relative()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(
                        count(entry.file.tokens, t.readable_muted(0.7))
                            .group_hover(group.clone(), |s| s.opacity(0.)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(px(-4.))
                            .flex()
                            .items_center()
                            .child(remove),
                    ),
            ),
    )
}

/// One of the tray's actions (Pink: pickers.rs `render_context_controls`
/// `compact` / `clear`): 30 tall, `px 8`, radius 6, gap 8, a 13 px glyph in
/// `text_muted`, the quiet wash on hover; half strength when unavailable.
/// One of the controls' 30 tall rows (Pink: `render_context_controls`'s
/// Compact and Clear): a 13 px muted glyph and the label, washed under the
/// pointer, at half strength when disabled, and lit while the Context
/// menu's arrows rest on it. A `focusable` row is its own tab stop (the
/// model menu's tray); the Context menu's rows follow its arrows instead.
fn tray_row(
    id: String,
    icon: &'static str,
    label: &'static str,
    enabled: bool,
    highlighted: bool,
    focusable: bool,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    let selector = id.clone();
    div()
        .id(eid(id.clone()))
        .debug_selector(move || selector)
        .role(Role::Button)
        .aria_label(label)
        .h(px(TRAY_ROW_H))
        .px(px(8.))
        .rounded(px(radius::SM))
        .flex()
        .items_center()
        .gap(px(8.))
        .when(highlighted, |row| row.bg(t.element_hover))
        .when(enabled, |row| {
            row.cursor_pointer()
                .hover_wash(id, t.fill(FillTier::Tertiary))
        })
        .when(enabled && focusable, |row| {
            row.tab_index(0)
                .focus_visible(move |style| pink::focus_ring(style, ring))
        })
        .when(!enabled, |row| row.opacity(0.5))
        .child(
            svg()
                .path(icon)
                .size(px(13.))
                .flex_none()
                .text_color(t.text_muted),
        )
        .child(label)
}

/// Clear's label (Pink: `clear_context_confirmation`).
fn clear_label(armed: bool) -> &'static str {
    if armed {
        "Confirm clear context"
    } else {
        "Clear context…"
    }
}

/// An 11 faint line in the controls (Pink: the tray heading and the
/// compact reason).
fn caption(text: SharedString, t: &Pink) -> Div {
    div()
        .px(px(8.))
        .pb(px(3.))
        .text_size(px(11.))
        .text_color(t.text_faint)
        .child(text)
}

/// The controls' column (Pink: `context-controls-tray`): inset by the card,
/// 6 above and below, rows 2 apart.
fn controls_panel(_: &Pink) -> Div {
    div()
        .flex_none()
        .px(px(layout::CARD_INSET))
        .py(px(6.))
        .flex()
        .flex_col()
        .gap(px(2.))
}

/// The Context menu's Compact and Clear (Pink: `render_context_controls`
/// while `PickerKind::Context` is open): no heading, Compact (its reason
/// when the provider can't, or when the history is too short to compact)
/// and Clear context, which asks to be confirmed.
fn tray(
    key: &str,
    s: &WindowState,
    props: &Rc<Props>,
    state: &Entity<WindowState>,
    t: &Pink,
) -> Div {
    let compacting = s.is_compacting();
    let compact_enabled = s.can_compact();
    let compact_reason = s.compact.reason().or_else(|| {
        (!compacting && !compact_enabled && s.compact.enabled())
            .then(|| "The chat history is too short to compact.".into())
    });
    let compact = {
        let state = state.clone();
        let props = props.clone();
        tray_row(
            format!("{key}-compact"),
            ICON_COMPACT,
            s.compact_label(),
            compact_enabled,
            s.active == Some(0),
            false,
            t,
        )
        .on_click(move |_, window, cx| activate(&state, &props, 0, window, cx))
    };
    let clear = {
        let state = state.clone();
        let props = props.clone();
        tray_row(
            format!("{key}-clear"),
            ICON_CLEAR,
            clear_label(s.confirm_clear),
            true,
            s.active == Some(1),
            false,
            t,
        )
        .on_click(move |_, window, cx| activate(&state, &props, 1, window, cx))
    };
    controls_panel(t)
        .child(compact)
        .when_some(compact_reason, |d, reason| {
            let selector = format!("{key}-compact-reason");
            d.child(caption(reason, t).debug_selector(move || selector))
        })
        .child(clear)
}

/// Compact (row 0) or Clear (row 1), from a click or Enter (Pink:
/// `compact_context`, `clear_context`).
fn activate(
    state: &Entity<WindowState>,
    props: &Rc<Props>,
    row: usize,
    window: &mut Window,
    cx: &mut App,
) {
    let fired = match row {
        0 => state.update(cx, |s, cx| s.start_compaction(cx)),
        1 => state.update(cx, |s, cx| s.clear(cx)),
        _ => false,
    };
    let handler = match row {
        0 => props.on_compact.as_ref(),
        _ => props.on_clear.as_ref(),
    };
    if fired && let Some(f) = handler {
        f(window, cx);
    }
}

/// The story's column: a composer under Pink's 768 maximum, with room
/// above it for the open card.
const STORY_W: f32 = 640.;
const STORY_H: f32 = 720.;

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .w(px(STORY_W))
            .h(px(STORY_H))
            .flex()
            .flex_col()
            .justify_between()
            .child(story_states(&t))
            .child(story_composer(&t))
    })
}

/// The chip in each of its states, on the composer's plate.
fn story_states(t: &Pink) -> Div {
    let chat = |id: &'static str, history: u32, turns: u32| {
        ContextWindow::new(id, "lumen-4", 200_000)
            .system(3_100)
            .tools(14_600)
            .file("rules", "billing_rules.md", 18_400)
            .conversation(history, turns)
            .memory(2_400)
            .reserve(8_000)
    };
    let sample = |chip: ContextWindow, caption: &'static str| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .child(chip)
            .child(
                div()
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .text_color(t.readable_muted(0.7))
                    .child(caption),
            )
    };
    div()
        .rounded(px(radius::COMPOSER_PILL))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
        .px(px(32.))
        .pt(px(10.))
        .pb(px(12.))
        .flex()
        .justify_between()
        .child(sample(chat("cw-calm", 46_000, 14), "Under 75%"))
        .child(sample(chat("cw-warm", 118_000, 38), "From 75%"))
        .child(sample(chat("cw-full", 150_000, 52), "From 90%"))
        .child(sample(
            chat("cw-busy", 118_000, 38).compacting(true),
            "Provider compacting",
        ))
        .child(sample(
            chat("cw-tight", 46_000, 14).show_label(false),
            "Tight row",
        ))
        // PINK's plain Context menu, for a provider that can't compact.
        .child(sample(
            chat("cw-plain", 46_000, 14)
                .window_map(false)
                .compact(CompactState::Unavailable(
                    "This provider has no verified compact operation.".into(),
                )),
            "Can't compact",
        ))
        .child(sample(
            chat("cw-new", 0, 0).unreported(true),
            "Not reported yet",
        ))
}

/// A composer with the chip beside the model picker and its Context menu
/// open (Pink: composer.rs controls row, opaque `composer_surface`).
fn story_composer(t: &Pink) -> Div {
    let attach = div()
        .id("story-attach")
        .flex_none()
        .size(px(layout::COMPOSER_CONTROL))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover_wash("story-attach", t.ink(0.10))
        .child(
            svg()
                .path(ICON_PAPERCLIP)
                .size(px(16.))
                .relative()
                .left(px(1.))
                .text_color(t.text_muted),
        );
    let model = pink::trigger_chip("story-model", true, false, t).child("lumen-4");
    let context = ContextWindow::new("cw-composer", "lumen-4", 200_000)
        .system(3_100)
        .tools(14_600)
        .file("rules", "billing_rules.md", 18_400)
        .file("schema", "invoice_schema.sql", 9_600)
        .file("tax", "q3_tax_changes.pdf", 13_200)
        .conversation(96_300, 46)
        .memory(2_400)
        .reserve(8_000)
        .note("Applies to the next turn on this thread.")
        .default_open(true);
    let send = div()
        .id("story-send")
        .flex_none()
        .size(px(layout::COMPOSER_CONTROL))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(motion::hover_blend(
            "story-send",
            t.text,
            t.text.opacity(0.88),
        ))
        .on_hover(motion::hover_listener("story-send"))
        .child(svg().path(ICON_ARROW_UP).size(px(14.)).text_color(t.bg));
    div()
        .flex_none()
        .rounded(px(radius::COMPOSER_PILL))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
        .shadow(pink::shadow_lg())
        .flex()
        .flex_col()
        .child(
            div()
                .min_h(px(60.))
                .px(px(16.))
                .pt(px(16.))
                .pb(px(4.))
                .text_size(px(14.))
                .line_height(px(22.75))
                .text_color(t.text)
                .child("Draft the dunning emails from the billing rules, then check them against the Q3 tax changes."),
        )
        .child(
            div()
                .h(px(46.))
                .px(px(10.))
                .pt(px(2.))
                .pb(px(12.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .child(attach)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(model)
                                .child(context),
                        ),
                )
                .child(send),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage() -> Usage {
        ContextWindow::new("ctx", "model", 200_000)
            .system(3_000)
            .file("a", "a.md", 10_000)
            .file("b", "b.md", 4_000)
            .conversation(20_000, 4)
            .reserve(8_000)
            .usage
    }

    #[test]
    fn cells_fill_in_order_with_the_reserve_last() {
        let cells = usage().cells();
        assert_eq!(cells.len(), 100);
        assert_eq!(cells[0], Cell::Part(Part::System, None));
        assert_eq!(cells[2], Cell::Part(Part::Files, Some(0)));
        assert_eq!(cells[7], Cell::Part(Part::Files, Some(1)));
        assert_eq!(cells[9], Cell::Part(Part::Conversation, None));
        assert_eq!(cells[19], Cell::Free);
        assert!(cells[96..].iter().all(|c| *c == Cell::Reserved));
    }

    #[test]
    fn removing_a_file_frees_its_cells() {
        let mut u = usage();
        u.files[0].removed = Some(Instant::now());
        assert_eq!(u.files_total(), 4_000);
        assert_eq!(u.cells()[2], Cell::Part(Part::Files, Some(1)));
        assert_eq!(u.tokens(Spot::Free), 200_000 - 27_000 - 8_000);
    }

    #[test]
    fn details_read_like_pinks_card() {
        assert_eq!(with_separators(950), "950");
        assert_eq!(with_separators(1_048_576), "1,048,576");
        assert_eq!(
            details(&usage()),
            "37,000 of 200,000 tokens\n163,000 tokens remaining"
        );
        assert_eq!(label(false, 0.786), "79%");
        assert_eq!(label(true, 0.786), "Compacting…");
    }

    /// PINK 0.30.6 (novnski/pink#193). Failure modes: an unreported window
    /// announced as "— used" or "0% used"; its tone a share's muted or
    /// warning; its card claiming a token count; compacting losing to it.
    #[test]
    fn unreported_usage_reads_as_words() {
        assert_eq!(
            accessible_name(false, false, "0%"),
            "Context window: usage not reported yet"
        );
        assert_eq!(
            accessible_name(false, true, "79%"),
            "Context window: 79% used"
        );
        assert_eq!(
            accessible_name(true, false, "Compacting…"),
            "Context window: compacting"
        );
        let t = Pink::dark();
        assert_eq!(chip_tone(false, false, 0.95, &t), t.text_faint);
        assert_eq!(chip_tone(true, false, 0.0, &t), t.accent);
        let mut unknown = usage();
        unknown.reported = false;
        assert_eq!(
            details(&unknown),
            "Context usage not reported by this harness yet"
        );
    }

    #[test]
    fn tone_follows_pinks_thresholds() {
        let t = Pink::dark();
        assert_eq!(tone(false, 0.5, &t), t.text_muted);
        assert_eq!(tone(false, 0.75, &t), t.warning);
        assert_eq!(tone(false, 0.9, &t), t.danger);
        assert_eq!(tone(true, 0.95, &t), t.accent);
    }
}
