//! OTP Input – a one-time code typed one character per cell, in Pink's
//! dialog-field chrome.
//!
//! A Pink component. Pink's only code entry is the sign-in dialog's
//! paste-code step (Pink: settings/accounts.rs `render_login_dialog`,
//! `LoginFlow::PasteCode`): a mono `dialog_field`, a plain red 12px line
//! under it when the code is refused, and the dialog's primary button
//! reading "Verifying…" at half opacity while the code is checked. This field
//! keeps that anatomy and splits the field into one cell per character:
//! every cell is a `dialog_field` plate (radius 8, `separator(false)` edge,
//! Quaternary fill), the active cell wears Pink's 2px inset focus ring, which
//! glides from cell to cell over `QUICK` like the segmented thumb, and an
//! empty active cell shows Pink's composer caret (2px accent, solid while
//! typing, blinking 500ms on / 500ms off when idle).
//!
//! The field keeps the code itself (or follows `value` when controlled):
//! typing replaces the character under the ring, Backspace removes it, the
//! arrows, Home and End move the ring, and a pasted "482 913" fills the cells
//! one after another. While verifying the cells are read-only and their
//! characters dim; a refused code prints the error line and clears for the
//! next try; an accepted one prints the success label behind a check. The
//! resend row counts down on a disabled ghost button that turns into
//! "Resend code" (Pink's pairing sheet: an "expires in m:ss" countdown and
//! `ghost_action` buttons, settings/connections.rs).
//!
//! ```ignore
//! OtpInput::new("otp")
//!     .separator(true)
//!     .on_complete(|code, _, _| println!("check {code}"))
//!     .on_resend(|_, _| {})
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use web_time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyView, App, Context, CursorStyle, ElementId, FocusHandle, IntoElement,
    KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, Pixels, Point, Role, SharedString, Task,
    Window, canvas, div, point, prelude::*, px,
};

use crate::{
    HoverFill,
    motion::{self, QUICK, STATUS_SWAP, TOOL_ROW_STAGGER_MS},
    pink::{
        self, ButtonKind, ControlSize, Pink, TextStyle, TypeStyleExt, glyphs, layout, loaders,
        radius, space,
    },
    story,
};

pub const NAME: &str = "OTP Input";
pub const DESCRIPTION: &str = "A one-time code typed one character per cell – dialog-field plates under a gliding focus ring, paste that fills every cell, a plain red line when the code is refused and a resend countdown";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type CodeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Largest cell; the default when no `fit_width` is given.
const CELL: f32 = 40.;
/// Cells grow to fill a `fit_width` up to this size, and shrink to
/// [`CELL_MIN`] (a `ControlSize::Md` control) at the narrowest.
const CELL_MAX: f32 = 44.;
const CELL_MIN: f32 = 28.;
const GAP: f32 = space::S8;
/// The dash between the two halves of the code.
const SEP_W: f32 = space::S8;
/// A refused code stays readable this long before it clears.
const CLEAR_AFTER: Duration = Duration::from_millis(700);

/// Where the code stands, set by the owner after checking it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OtpStatus {
    #[default]
    Idle,
    /// The cells are read-only and their characters dim (Pink keeps the field
    /// and relabels the dialog's primary button "Verifying…").
    Verifying,
    /// The error line under the cells; the code clears for a retry.
    Error,
    /// The success line under the cells; the field stays read-only.
    Success,
}

/// Placement of the cells and the lines below them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OtpAlign {
    #[default]
    Start,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Separator {
    None,
    Middle,
    After(usize),
}

/// "0:24" (Pink's pairing sheet "expires in {}:{:02}").
fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Keeps the allowed characters only – the spaces and dashes of a pasted
/// "482 913" fall away; letters (when allowed) turn into capitals.
fn clean_code(text: &str, numeric: bool, length: usize) -> String {
    text.chars()
        .filter_map(|c| {
            if numeric {
                c.is_ascii_digit().then_some(c)
            } else {
                c.is_ascii_alphanumeric().then(|| c.to_ascii_uppercase())
            }
        })
        .take(length)
        .collect()
}

/// Typing with the active cell selected: the character under the ring is
/// replaced (or appended in the first empty cell). `None` when nothing
/// changes.
fn type_into(
    code: &str,
    active: usize,
    typed: &str,
    numeric: bool,
    length: usize,
) -> Option<(String, usize)> {
    let raw = if active < code.len() {
        format!("{}{}{}", &code[..active], typed, &code[active + 1..])
    } else {
        format!("{code}{typed}")
    };
    let next = clean_code(&raw, numeric, length);
    // The caret counts the kept characters in front of it
    let caret = clean_code(
        &raw[..active.min(code.len()) + typed.len()],
        numeric,
        length,
    )
    .len();
    (next != code).then_some((next, caret))
}

/// A paste: a whole code replaces everything; a part is written from the
/// active cell on.
fn paste_into(
    code: &str,
    active: usize,
    pasted: &str,
    numeric: bool,
    length: usize,
) -> Option<(String, usize)> {
    let text = clean_code(pasted, numeric, length);
    if text.is_empty() {
        return None;
    }
    if text.len() >= length {
        return Some((text, length - 1));
    }
    let active = active.min(code.len());
    let mut next = code[..active].to_string() + &text;
    next.push_str(code.get(active + text.len()..).unwrap_or(""));
    next.truncate(length);
    Some((next, active + text.len()))
}

/// Backspace or Delete: the active character is selected, so both remove
/// it; in an empty cell Backspace takes the one before.
fn delete_at(code: &str, active: usize, backspace: bool) -> Option<(String, usize)> {
    let mut next = code.to_string();
    if active < code.len() {
        next.remove(active);
        Some((next, active))
    } else if backspace && active > 0 {
        next.remove(active - 1);
        Some((next, active - 1))
    } else {
        None
    }
}

/// Pink's caret cadence (composer.rs `caret_visible`): solid for the first
/// half-period after any keystroke or caret move, then 500ms off, 500ms on.
fn caret_visible(ms_since_activity: u64) -> bool {
    (ms_since_activity / layout::CARET_BLINK_MS).is_multiple_of(2)
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// Where the cells sit, from the width the field lays out in.
#[derive(Clone)]
struct Layout {
    n: usize,
    size: f32,
    sep_at: Option<usize>,
    width: f32,
}

impl Layout {
    fn new(n: usize, sep_at: Option<usize>, fit: Option<f32>) -> Self {
        let sep_space = if sep_at.is_some() { GAP + SEP_W } else { 0. };
        let fixed = (n as f32 - 1.) * GAP + sep_space;
        let size = match fit {
            Some(w) => ((w - fixed) / n as f32).clamp(CELL_MIN, CELL_MAX),
            None => CELL,
        };
        Layout {
            n,
            size,
            sep_at,
            width: n as f32 * size + fixed,
        }
    }

    fn cell_x(&self, i: usize) -> f32 {
        let after_sep = self.sep_at.is_some_and(|k| i >= k);
        i as f32 * (self.size + GAP) + if after_sep { GAP + SEP_W } else { 0. }
    }

    fn sep_x(&self) -> Option<f32> {
        self.sep_at.map(|k| k as f32 * (self.size + GAP))
    }

    /// A click picks the cell under the pointer (4px of slack to its right).
    fn pick(&self, x: f32) -> usize {
        (0..self.n)
            .find(|&i| x < self.cell_x(i) + self.size + 4.)
            .unwrap_or(self.n - 1)
    }

    /// The characters' type on the ladder: Title2 17/22 in full-size cells,
    /// Title3 15/20 and Body 13/18 (Pink's code size) as they shrink.
    fn style(&self) -> TextStyle {
        if self.size >= CELL {
            TextStyle::Title2
        } else if self.size >= 32. {
            TextStyle::Title3
        } else {
            TextStyle::Body
        }
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// What a change asks the owner to hear about.
#[derive(Default)]
struct Effects {
    change: Option<SharedString>,
    complete: Option<SharedString>,
}

impl Effects {
    fn send(
        self,
        on_change: &Option<CodeHandler>,
        on_complete: &Option<CodeHandler>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let (Some(code), Some(f)) = (self.change, on_change) {
            f(&code, window, cx);
        }
        if let (Some(code), Some(f)) = (self.complete, on_complete) {
            f(&code, window, cx);
        }
    }
}

struct OtpState {
    focus: FocusHandle,
    /// The code as of the last render or edit.
    code: String,
    /// The code when uncontrolled.
    inner: String,
    /// The cell the next character goes to.
    idx: usize,
    error_shown: bool,
    /// The refused code fades out.
    clearing: bool,
    /// No typing between the error and the clearing.
    locked: bool,
    /// Seconds until Resend.
    left: u32,
    resend_in: u32,
    prev_code: String,
    /// The code this field's own last edit produced: any other new code
    /// came from the owner, and the ring moves to its end.
    edited: Option<String>,
    /// Per cell: when its character arrived (keys its entrance) and its
    /// stagger.
    stamps: Vec<u64>,
    delays: Vec<u64>,
    counter: u64,
    prev_status: OtpStatus,
    /// The last keystroke or caret move: the caret stays solid after it.
    blink_anchor: Instant,
    /// The cells' top-left on screen, for picking a cell under the pointer.
    origin: Rc<Cell<Point<Pixels>>>,
    // The latest props, for timers and listeners.
    layout: Option<Layout>,
    numeric: bool,
    controlled: bool,
    on_change: Option<CodeHandler>,
    clear_task: Option<Task<()>>,
    countdown: Option<Task<()>>,
    /// Repaints every half-period while the caret can blink (Pink's
    /// `blink_task`); dropped on blur so an idle field schedules nothing.
    blink_task: Option<Task<()>>,
}

impl OtpState {
    fn new(props: &OtpInput, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle().tab_stop(true);
        if props.auto_focus && !props.disabled {
            window.focus(&focus, cx);
        }
        let code = clean_code(
            props.value.as_ref().unwrap_or(&props.default_value),
            props.numeric,
            props.length.max(1),
        );
        let mut state = OtpState {
            focus,
            idx: code.len().min(props.length.max(1) - 1),
            inner: code.clone(),
            prev_code: code.clone(),
            edited: None,
            code,
            error_shown: false,
            clearing: false,
            locked: false,
            left: props.resend_in,
            resend_in: props.resend_in,
            stamps: Vec::new(),
            delays: Vec::new(),
            counter: 0,
            prev_status: props.status,
            blink_anchor: Instant::now(),
            origin: Rc::new(Cell::new(point(px(0.), px(0.)))),
            layout: None,
            numeric: props.numeric,
            controlled: props.value.is_some(),
            on_change: None,
            clear_task: None,
            countdown: None,
            blink_task: None,
        };
        state.start_countdown(window, cx);
        state
    }

    fn length(&self) -> usize {
        self.layout.as_ref().map_or(1, |l| l.n)
    }

    fn active(&self) -> usize {
        self.idx.min(self.code.len()).min(self.length() - 1)
    }

    fn start_countdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.left == 0 {
            self.countdown = None;
            return;
        }
        self.countdown = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let more = this.update(cx, |s, cx| {
                    s.left = s.left.saturating_sub(1);
                    cx.notify();
                    s.left > 0
                });
                if !matches!(more, Ok(true)) {
                    break;
                }
            }
        }));
    }

    /// Keeps the caret blinking while it can show (Pink `caret_shown`).
    fn arm_blink(&mut self, on: bool, cx: &mut Context<Self>) {
        if !on {
            self.blink_task = None;
            return;
        }
        if self.blink_task.is_none() {
            self.blink_task = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(layout::CARET_BLINK_MS))
                        .await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                }
            }));
        }
    }

    fn commit(&mut self, next: String, next_idx: usize, user: bool) -> Effects {
        let n = self.length();
        let changed = next != self.code;
        if !self.controlled {
            self.inner = next.clone();
        }
        let mut effects = Effects::default();
        if changed {
            effects.change = Some(next.clone().into());
        }
        if user {
            if self.error_shown && !next.is_empty() {
                self.error_shown = false;
            }
            if next.len() == n && changed {
                effects.complete = Some(next.clone().into());
            }
        }
        self.idx = next_idx.min(next.len()).min(n - 1);
        self.edited = Some(next.clone());
        self.code = next;
        self.blink_anchor = Instant::now();
        effects
    }

    /// One key press; `None` lets it bubble on.
    fn key(
        &mut self,
        keystroke: &Keystroke,
        blocked: bool,
        cx: &mut Context<Self>,
    ) -> Option<Effects> {
        let n = self.length();
        let len = self.code.len();
        let active = self.active();
        let read_only = blocked || self.locked;
        let mods = keystroke.modifiers;
        let key = keystroke.key.as_str();

        if (mods.secondary() || mods.control) && !mods.alt && key == "v" {
            if read_only {
                return Some(Effects::default());
            }
            let pasted = cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .unwrap_or_default();
            return Some(
                match paste_into(&self.code, active, &pasted, self.numeric, n) {
                    Some((next, caret)) => self.commit(next, caret, true),
                    None => Effects::default(),
                },
            );
        }

        if !mods.shift && !mods.platform && !mods.control {
            let last = len.min(n - 1);
            let target = match key {
                "left" => Some(active.saturating_sub(1)),
                "right" => Some(active + 1),
                "home" | "up" => Some(0),
                "end" | "down" => Some(last),
                _ => None,
            };
            if let Some(target) = target {
                self.idx = target.min(last);
                self.blink_anchor = Instant::now();
                return Some(Effects::default());
            }
        }

        match key {
            "backspace" | "delete" => {
                if read_only {
                    return Some(Effects::default());
                }
                return Some(match delete_at(&self.code, active, key == "backspace") {
                    Some((next, caret)) => self.commit(next, caret, true),
                    None => Effects::default(),
                });
            }
            "enter" | "tab" | "escape" => return None,
            _ => {}
        }

        if mods.control || mods.platform || mods.alt || mods.function {
            return None;
        }
        let typed = keystroke.key_char.as_ref()?;
        if read_only {
            return Some(Effects::default());
        }
        Some(
            match type_into(&self.code, active, typed, self.numeric, n) {
                Some((next, caret)) => self.commit(next, caret, true),
                None => Effects::default(),
            },
        )
    }

    /// A new status: an error shows its line and, after a beat, clears the
    /// code for the next try.
    fn status_changed(
        &mut self,
        status: OtpStatus,
        clear_on_error: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prev_status = status;
        if status != OtpStatus::Error {
            self.error_shown = false;
            return;
        }
        self.error_shown = true;
        if !clear_on_error {
            return;
        }
        self.locked = true;
        let fade = motion::exit_of(STATUS_SWAP)
            .total()
            .mul_f32(motion::speed_scale());
        self.clear_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(CLEAR_AFTER).await;
            if this
                .update(cx, |s, cx| {
                    s.clearing = true;
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
            cx.background_executor().timer(fade).await;
            let Ok((on_change, effects)) = this.update(cx, |s, cx| {
                s.clearing = false;
                s.locked = false;
                let effects = s.commit(String::new(), 0, false);
                cx.notify();
                (s.on_change.clone(), effects)
            }) else {
                return;
            };
            cx.update(|window, cx| effects.send(&on_change, &None, window, cx))
                .ok();
        }));
    }
}

/// The line under the cells.
#[derive(Clone, PartialEq)]
enum Note {
    Error(SharedString),
    Success(SharedString),
}

/// Everything one frame draws, read out of the state.
struct Frame {
    code: String,
    active: usize,
    focused: bool,
    read_only: bool,
    clearing: bool,
    caret: bool,
    stamps: Vec<u64>,
    delays: Vec<u64>,
    left: u32,
    note: Option<Note>,
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct OtpInput {
    id: ElementId,
    length: usize,
    separator: Separator,
    numeric: bool,
    value: Option<SharedString>,
    default_value: SharedString,
    on_change: Option<CodeHandler>,
    on_complete: Option<CodeHandler>,
    status: OtpStatus,
    error_message: SharedString,
    clear_on_error: bool,
    success_label: SharedString,
    on_resend: Option<Handler>,
    resend_in: u32,
    align: OtpAlign,
    disabled: bool,
    auto_focus: bool,
    fit_width: Option<Pixels>,
}

impl OtpInput {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            length: 6,
            separator: Separator::None,
            numeric: true,
            value: None,
            default_value: SharedString::default(),
            on_change: None,
            on_complete: None,
            status: OtpStatus::Idle,
            error_message: "That code didn't work. Try again.".into(),
            clear_on_error: true,
            success_label: "Verified".into(),
            on_resend: None,
            resend_in: 30,
            align: OtpAlign::Start,
            disabled: false,
            auto_focus: false,
            fit_width: None,
        }
    }

    /// Number of cells (6).
    pub fn length(mut self, length: usize) -> Self {
        self.length = length.max(1);
        self
    }

    /// A small dash between the cells, in the middle.
    pub fn separator(mut self, on: bool) -> Self {
        self.separator = if on {
            Separator::Middle
        } else {
            Separator::None
        };
        self
    }

    /// A small dash after `cells` cells.
    pub fn separator_after(mut self, cells: usize) -> Self {
        self.separator = Separator::After(cells);
        self
    }

    /// Digits only (default); `false` allows letters too, shown in capitals.
    pub fn numeric(mut self, numeric: bool) -> Self {
        self.numeric = numeric;
        self
    }

    /// The code – makes the field controlled; pair it with `on_change`.
    pub fn value(mut self, code: impl Into<SharedString>) -> Self {
        self.value = Some(code.into());
        self
    }

    /// Starting code when uncontrolled.
    pub fn default_value(mut self, code: impl Into<SharedString>) -> Self {
        self.default_value = code.into();
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Called once the last cell is filled – typed or pasted.
    pub fn on_complete(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_complete = Some(Rc::new(handler));
        self
    }

    pub fn status(mut self, status: OtpStatus) -> Self {
        self.status = status;
        self
    }

    /// Shown under the cells while the status is `Error`.
    pub fn error_message(mut self, message: impl Into<SharedString>) -> Self {
        self.error_message = message.into();
        self
    }

    /// Clears the cells shortly after an error, ready for the next try (on).
    pub fn clear_on_error(mut self, clear: bool) -> Self {
        self.clear_on_error = clear;
        self
    }

    /// Shown under the cells once the code is accepted ("Verified").
    pub fn success_label(mut self, label: impl Into<SharedString>) -> Self {
        self.success_label = label.into();
        self
    }

    /// Shows the resend row under the cells: a countdown, then "Resend code".
    pub fn on_resend(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_resend = Some(Rc::new(handler));
        self
    }

    /// Seconds until the code can be resent (30).
    pub fn resend_in(mut self, seconds: u32) -> Self {
        self.resend_in = seconds;
        self
    }

    pub fn align(mut self, align: OtpAlign) -> Self {
        self.align = align;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn auto_focus(mut self, auto_focus: bool) -> Self {
        self.auto_focus = auto_focus;
        self
    }

    /// The width the field lays out in: the cells grow or shrink to fill it,
    /// like Pink's full-width `dialog_field`. Unset, the cells are 40px.
    pub fn fit_width(mut self, width: Pixels) -> Self {
        self.fit_width = Some(width);
        self
    }

    /// Reads the state for this frame.
    fn sync(
        &self,
        s: &mut OtpState,
        layout: &Layout,
        window: &mut Window,
        cx: &mut Context<OtpState>,
    ) -> Frame {
        let n = layout.n;
        s.layout = Some(layout.clone());
        s.numeric = self.numeric;
        s.controlled = self.value.is_some();
        s.on_change = self.on_change.clone();
        s.resend_in = self.resend_in;
        s.code = match &self.value {
            Some(value) => clean_code(value, self.numeric, n),
            None => clean_code(&s.inner, self.numeric, n),
        };

        // Every character remembers when it arrived, so it settles in once;
        // a pasted code arrives one cell after another.
        if s.prev_code != s.code {
            let before: Vec<char> = s.prev_code.chars().collect();
            let changed: Vec<bool> = s
                .code
                .chars()
                .enumerate()
                .map(|(i, c)| before.get(i) != Some(&c))
                .collect();
            let first = changed.iter().position(|&c| c).unwrap_or(0);
            let bulk = changed.iter().filter(|&&c| c).count() > 1;
            s.stamps.resize(n, 0);
            s.delays.resize(n, 0);
            for (i, &c) in changed.iter().enumerate() {
                if c {
                    s.counter += 1;
                    s.stamps[i] = s.counter;
                    s.delays[i] = if bulk {
                        (i - first) as u64 * TOOL_ROW_STAGGER_MS
                    } else {
                        0
                    };
                }
            }
            if s.edited.as_deref() != Some(s.code.as_str()) {
                s.idx = s.code.len().min(n - 1);
            }
            s.prev_code = s.code.clone();
            s.blink_anchor = Instant::now();
        }
        s.edited = None;

        if self.status != s.prev_status {
            s.status_changed(self.status, self.clear_on_error, window, cx);
        }

        let busy = matches!(self.status, OtpStatus::Verifying | OtpStatus::Success);
        let read_only = busy || s.locked || self.disabled;
        let focused = !self.disabled && s.focus.is_focused(window);
        let active = s.active();
        let empty_cell = s.code.chars().nth(active).is_none();
        let can_blink = focused && !read_only && empty_cell && window.is_window_active();
        s.arm_blink(can_blink, cx);
        let since = s.blink_anchor.elapsed().as_millis() as u64;

        let note = match self.status {
            OtpStatus::Error if s.error_shown => Some(Note::Error(self.error_message.clone())),
            OtpStatus::Success => Some(Note::Success(self.success_label.clone())),
            _ => None,
        };
        Frame {
            code: s.code.clone(),
            active,
            focused,
            read_only,
            clearing: s.clearing,
            caret: can_blink && caret_visible(since),
            stamps: s.stamps.clone(),
            delays: s.delays.clone(),
            left: s.left,
            note,
        }
    }
}

impl RenderOnce for OtpInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let reduced = motion::reduced_motion(cx);
        let n = self.length.max(1);
        let sep_at = match self.separator {
            Separator::None => None,
            Separator::Middle => Some(n / 2),
            Separator::After(k) => Some(k),
        }
        .filter(|&k| k > 0 && k < n);
        let layout = Layout::new(n, sep_at, self.fit_width.map(f32::from));
        let key = self.id.to_string();

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| OtpState::new(&self, window, cx),
        );
        let f = state.update(cx, |s, cx| self.sync(s, &layout, window, cx));
        let (focus_handle, origin) = {
            let s = state.read(cx);
            (s.focus.clone(), s.origin.clone())
        };
        let center = self.align == OtpAlign::Center;
        let verifying = self.status == OtpStatus::Verifying;
        let blocked =
            matches!(self.status, OtpStatus::Verifying | OtpStatus::Success) || self.disabled;

        let size = layout.size;
        let style = layout.style();
        let (font, line) = (style.size(), style.line_height());
        // Pink's busy field: the characters drop to the description tone
        let ink = if verifying {
            t.readable_muted(0.85)
        } else {
            t.text
        };

        // ----- Cells: one dialog_field plate per character -----
        // Pink: popover.rs `dialog_field`, the code in the theme's mono font
        // (accounts.rs `render_login_dialog`)
        let mut cells = Vec::with_capacity(n);
        for i in 0..n {
            let mut cell = pink::dialog_field(&t)
                .absolute()
                .left(px(layout.cell_x(i)))
                .top_0()
                .w(px(size))
                .h(px(size))
                .p_0()
                .flex()
                .items_center()
                .justify_center()
                .font_family(pink::MONO)
                .text_size(px(font))
                .line_height(px(line))
                .text_color(ink);
            if let Some(ch) = f.code.chars().nth(i) {
                let stamp = f.stamps.get(i).copied().unwrap_or(0);
                let glyph = div().relative().child(ch.to_string());
                cell = if reduced {
                    cell.child(glyph.when(f.clearing, |d| d.opacity(0.)))
                } else if f.clearing {
                    // The refused code leaves over Pink's exit of the status swap
                    cell.child(glyph.with_animation(
                        ElementId::Name(format!("out-{i}-{stamp}").into()),
                        motion::exit_of(STATUS_SWAP).animation(),
                        |el, t| el.opacity(1. - t),
                    ))
                } else {
                    // A character arrives like Pink's status label: a fade
                    // and a 2px settle over STATUS_SWAP
                    let delay = f.delays.get(i).copied().unwrap_or(0);
                    cell.child(glyph.with_animation(
                        ElementId::Name(format!("in-{i}-{stamp}").into()),
                        STATUS_SWAP.with_delay(delay).animation(),
                        |el, t| el.opacity(t).top(px(2. * (1. - t))),
                    ))
                };
            }
            cells.push(cell);
        }

        let sep = layout.sep_x().map(|x| {
            div()
                .absolute()
                .left(px(x))
                .top(px((size - 1.) / 2.))
                .w(px(SEP_W))
                .h(px(1.))
                .bg(t.decorative_muted(0.5))
        });

        // ----- The focus ring and the caret -----
        // Pink: theme.rs `focus_ring` (2px inset), gliding like the
        // segmented thumb over QUICK
        let show_ring = f.focused && !f.read_only;
        let ring_x = motion::follow(&format!("{key}-ring-x"), layout.cell_x(f.active), QUICK);
        let ring_on = motion::follow(
            &format!("{key}-ring"),
            if show_ring { 1. } else { 0. },
            QUICK,
        );
        let ring = (ring_on > 0.).then(|| {
            div()
                .absolute()
                .left(px(ring_x))
                .top_0()
                .size(px(size))
                .rounded(px(radius::MD))
                .shadow(t.focus_ring_shadows())
                .opacity(ring_on)
        });
        // Pink: composer.rs caret quad, 2px wide and one line tall
        let caret = f.caret.then(|| {
            div()
                .absolute()
                .left(px(ring_x + (size - layout::CARET_WIDTH) / 2.))
                .top(px((size - line) / 2.))
                .w(px(layout::CARET_WIDTH))
                .h(px(line))
                .bg(t.caret)
        });

        // ----- Input: keys, paste, picking a cell -----
        let on_change = self.on_change.clone();
        let on_complete = self.on_complete.clone();
        let key_state = state.clone();
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let effects = key_state.update(cx, |s, cx| {
                let effects = s.key(&event.keystroke, blocked, cx);
                if effects.is_some() {
                    cx.notify();
                }
                effects
            });
            if let Some(effects) = effects {
                cx.stop_propagation();
                effects.send(&on_change, &on_complete, window, cx);
            }
        };
        let pick_state = state.clone();
        let pick_focus = focus_handle.clone();
        let pick_origin = origin.clone();
        let disabled = self.disabled;
        let on_pick = move |event: &MouseDownEvent, window: &mut Window, cx: &mut App| {
            if disabled {
                return;
            }
            window.focus(&pick_focus, cx);
            let x = f32::from(event.position.x - pick_origin.get().x);
            pick_state.update(cx, |s, cx| {
                if let Some(layout) = &s.layout {
                    s.idx = layout.pick(x).min(s.code.len()).min(layout.n - 1);
                }
                s.blink_anchor = Instant::now();
                cx.notify();
            });
        };
        let blur_focus = focus_handle.clone();
        let tracker = canvas(
            move |bounds, _, _| origin.set(bounds.origin),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let field = div()
            .id("cells")
            .role(Role::TextInput)
            .aria_label("Verification code")
            .relative()
            .flex_none()
            .w(px(layout.width))
            .h(px(size))
            .when(!self.disabled, |d| d.track_focus(&focus_handle))
            .cursor(if f.read_only {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, on_pick)
            .on_mouse_down_out(move |_, window, cx| {
                if blur_focus.is_focused(window) {
                    window.blur(cx);
                }
            })
            .children(cells)
            .children(sep)
            .children(ring)
            .children(caret)
            .child(tracker);

        // ----- The line under the cells -----
        // Pink: the paste-code error, `mt 8`, 12px in `danger_muted` at 90%
        // (accounts.rs `render_login_dialog`); success mirrors it in
        // `success_muted` behind a check, one glyph and one word
        let note = f.note.map(|note| {
            let id = match &note {
                Note::Error(text) => loaders::label_id(&format!("{key}-error"), text),
                Note::Success(text) => loaders::label_id(&format!("{key}-ok"), text),
            };
            let line = div()
                .flex()
                .items_center()
                .gap(px(space::S6))
                .when(center, |d| d.justify_center())
                .type_style(TextStyle::Footnote);
            let line = match note {
                Note::Error(text) => line.text_color(t.danger_muted.opacity(0.9)).child(text),
                Note::Success(text) => line
                    .text_color(t.success_muted.opacity(0.9))
                    .child(
                        pink::line_glyph(
                            glyphs::CHECK,
                            pink::icon_size::SM,
                            t.readable_icon(t.success),
                        )
                        .flex_none()
                        .size(px(pink::icon_size::SM)),
                    )
                    .child(text),
            };
            div()
                .w_full()
                .mt(px(space::S8))
                .child(motion::status_swap(id, line))
        });

        // ----- Resend -----
        // Pink: the pairing sheet's countdown and `ghost_action` buttons
        // (connections.rs), a disabled ghost counting down, then enabled
        let resend = self.on_resend.clone().map(|on_resend| {
            let can = f.left == 0 && !blocked;
            let label: SharedString = if f.left > 0 {
                format!("Resend in {}", clock(f.left)).into()
            } else {
                "Resend code".into()
            };
            let resend_state = state.clone();
            let button = pink::button_enabled(
                format!("{key}-resend"),
                ButtonKind::Ghost,
                ControlSize::Md,
                can,
                &t,
            )
            .aria_label(label.clone())
            .font_features(pink::tabular_figures())
            .child(label)
            .when(can, |b| {
                b.on_click(move |_, window, cx| {
                    resend_state.update(cx, |s, cx| {
                        s.left = s.resend_in;
                        s.start_countdown(window, cx);
                        cx.notify();
                    });
                    on_resend(window, cx);
                })
            });
            div()
                .mt(px(space::S12))
                .flex()
                .items_center()
                .gap(px(space::S8))
                .when(center, |d| d.justify_center())
                .child(
                    div()
                        .type_style(TextStyle::Footnote)
                        .text_color(t.readable_muted(0.85))
                        .child("Didn't get a code?"),
                )
                .child(button)
        });

        div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .when(center, |d| d.items_center())
            .map(|d| match self.fit_width {
                Some(w) => d.w(w),
                None => d.w_full(),
            })
            .text_color(t.text)
            .when(self.disabled, |d| d.opacity(0.45))
            .child(
                div()
                    .flex()
                    .w_full()
                    .min_w_0()
                    .when(center, |d| d.justify_center())
                    .child(field),
            )
            .children(note)
            .children(resend)
    }
}

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

const DEMO_CODE: &str = "482913";
const DEMO_WRONG: &str = "481973";
/// The field fills the dialog's content column, inside its 1px edge.
const FIELD_W: f32 = layout::DIALOG_WIDTH - 2. * layout::DIALOG_PAD - 2.;

fn tries_message(tries: u32) -> String {
    if tries > 0 {
        let word = if tries == 1 { "try" } else { "tries" };
        format!("That code didn't work. {tries} {word} left.")
    } else {
        "That code didn't work. Request a new code.".into()
    }
}

/// The demo: types a wrong code, then the right one, and checks each; any
/// click or key takes over from the auto-play.
struct DemoState {
    code: SharedString,
    status: OtpStatus,
    tries: u32,
    playing: bool,
    timers: Vec<Task<()>>,
}

impl DemoState {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut demo = DemoState {
            code: SharedString::default(),
            status: OtpStatus::Idle,
            tries: 3,
            playing: true,
            timers: Vec::new(),
        };
        demo.play(cx);
        demo
    }

    fn after(
        &mut self,
        ms: u64,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        self.timers.push(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            this.update(cx, |s, cx| {
                f(s, cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// The server check: a second of verifying, then the verdict.
    fn verify(&mut self, code: SharedString, cx: &mut Context<Self>) {
        if self.status == OtpStatus::Verifying || code.len() != DEMO_CODE.len() {
            return;
        }
        self.status = OtpStatus::Verifying;
        self.after(1000, cx, move |s, cx| {
            if code.as_ref() == DEMO_CODE {
                s.status = OtpStatus::Success;
                s.after(3200, cx, |s, _| s.reset());
            } else {
                s.tries = s.tries.saturating_sub(1);
                s.status = OtpStatus::Error;
            }
        });
    }

    fn reset(&mut self) {
        self.code = SharedString::default();
        self.status = OtpStatus::Idle;
        self.tries = 3;
    }

    fn type_code(&mut self, code: &'static str, at: &mut u64, cx: &mut Context<Self>) {
        for end in 1..=code.len() {
            let prefix = SharedString::from(&code[..end]);
            self.after(*at, cx, move |s, _| s.code = prefix);
            *at += 150;
        }
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        let mut at = 700;
        self.type_code(DEMO_WRONG, &mut at, cx);
        self.after(at + 150, cx, |s, cx| s.verify(DEMO_WRONG.into(), cx));
        at += 2850;
        self.after(at, cx, |s, _| s.status = OtpStatus::Idle);
        self.type_code(DEMO_CODE, &mut at, cx);
        self.after(at + 150, cx, |s, cx| {
            s.playing = false;
            s.verify(DEMO_CODE.into(), cx);
        });
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if !self.playing {
            return;
        }
        self.playing = false;
        self.timers.clear();
        self.reset();
        cx.notify();
    }
}

/// Pink's paste-code sign-in dialog (accounts.rs `render_login_dialog`):
/// title, body copy, a "Reopen…"-style text link, the field at `mt 12`, then
/// Cancel and a primary that reads "Verifying…" at half opacity while the
/// code is checked.
#[derive(IntoElement)]
struct OtpDemo;

impl RenderOnce for OtpDemo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("otp-demo", cx, |_, cx| DemoState::new(cx));
        let (code, status, tries) = {
            let s = demo.read(cx);
            (s.code.clone(), s.status, s.tries)
        };
        let verifying = status == OtpStatus::Verifying;
        // A refused code is on its way out, so only a fresh full code submits
        let ready = code.len() == DEMO_CODE.len() && status == OtpStatus::Idle;

        let otp = OtpInput::new("otp")
            .separator(true)
            .value(code.clone())
            .status(status)
            .error_message(tries_message(tries))
            .resend_in(30)
            .fit_width(px(FIELD_W))
            .auto_focus(true)
            .on_change({
                let demo = demo.clone();
                move |code, _, cx| {
                    demo.update(cx, |s, cx| {
                        s.code = code.clone();
                        if s.status == OtpStatus::Error && !code.is_empty() {
                            s.status = OtpStatus::Idle;
                        }
                        cx.notify();
                    })
                }
            })
            .on_complete({
                let demo = demo.clone();
                move |code, _, cx| {
                    demo.update(cx, |s, cx| {
                        s.verify(code.clone(), cx);
                        cx.notify();
                    })
                }
            })
            .on_resend({
                let demo = demo.clone();
                move |_, cx| {
                    demo.update(cx, |s, cx| {
                        s.tries = 3;
                        cx.notify();
                    })
                }
            });

        // Pink: accounts.rs `url_link` – 12px, muted, brightening on hover
        let change = div()
            .id("otp-change-email")
            .mt(px(space::S6))
            .text_size(px(pink::text::FOOTNOTE))
            .truncate()
            .cursor_pointer()
            .hover_fg("otp-change-email", t.text_muted, t.text)
            .on_click({
                let demo = demo.clone();
                move |_, _, cx| {
                    demo.update(cx, |s, cx| {
                        s.reset();
                        cx.notify();
                    })
                }
            })
            .child("Use a different email");

        let cancel = pink::button("otp-cancel", ButtonKind::Ghost, ControlSize::Lg, &t)
            .child("Cancel")
            .on_click({
                let demo = demo.clone();
                move |_, _, cx| {
                    demo.update(cx, |s, cx| {
                        s.reset();
                        cx.notify();
                    })
                }
            });
        let submit = pink::button_enabled(
            "otp-verify",
            ButtonKind::Primary,
            ControlSize::Lg,
            ready || verifying,
            &t,
        )
        .child(if verifying { "Verifying…" } else { "Verify" })
        .when(verifying, |b| b.opacity(0.5))
        .when(ready, |b| {
            let demo = demo.clone();
            b.on_click(move |_, _, cx| {
                demo.update(cx, |s, cx| {
                    s.verify(code.clone(), cx);
                    cx.notify();
                })
            })
        });

        let card = pink::dialog_card(&t)
            .child(pink::dialog_title("Check your email", &t))
            .child(div().mt(px(space::S8)).child(pink::dialog_body(
                "We sent a 6-digit code to maya@acme.com. Enter it below to finish \
                 signing in; the code works once and expires in 10 minutes.",
                &t,
            )))
            .child(change)
            .child(div().mt(px(space::S12)).child(otp))
            .child(
                div()
                    .mt(px(space::S16))
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(space::S8))
                    .child(cancel)
                    .child(submit),
            );

        let stop_mouse = demo.clone();
        let stop_key = demo.clone();
        div()
            .id("otp-demo")
            .font_family(pink::SANS)
            .capture_any_mouse_down(move |_, _, cx| stop_mouse.update(cx, |s, cx| s.stop(cx)))
            .capture_key_down(move |_, _, cx| stop_key.update(cx, |s, cx| s.stop(cx)))
            .child(motion::dialog_in("otp-dialog", card))
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| OtpDemo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_pasted_codes() {
        assert_eq!(clean_code("482 913", true, 6), "482913");
        assert_eq!(clean_code("48-29-13-77", true, 6), "482913");
        assert_eq!(clean_code("ab-12", false, 6), "AB12");
        assert_eq!(clean_code("ab12", true, 6), "12");
    }

    #[test]
    fn typing_replaces_the_active_cell() {
        assert_eq!(type_into("", 0, "4", true, 6), Some(("4".into(), 1)));
        assert_eq!(type_into("481", 1, "2", true, 6), Some(("421".into(), 2)));
        assert_eq!(type_into("481", 3, "x", true, 6), None);
        assert_eq!(
            type_into("482910", 5, "3", true, 6),
            Some(("482913".into(), 6))
        );
    }

    #[test]
    fn pasting_fills_from_the_active_cell() {
        assert_eq!(
            paste_into("12", 1, "482 913", true, 6),
            Some(("482913".into(), 5))
        );
        assert_eq!(paste_into("12", 2, "34", true, 6), Some(("1234".into(), 4)));
        assert_eq!(
            paste_into("1299", 2, "34", true, 6),
            Some(("1234".into(), 4))
        );
        assert_eq!(paste_into("12", 1, "--", true, 6), None);
    }

    #[test]
    fn deleting_removes_the_selected_character() {
        assert_eq!(delete_at("481", 1, true), Some(("41".into(), 1)));
        assert_eq!(delete_at("481", 3, true), Some(("48".into(), 2)));
        assert_eq!(delete_at("481", 3, false), None);
        assert_eq!(delete_at("", 0, true), None);
    }

    #[test]
    fn cells_fill_the_field_and_make_room_for_the_separator() {
        // The demo dialog's content column
        let l = Layout::new(6, Some(3), Some(FIELD_W));
        assert!((l.width - FIELD_W).abs() < 0.01);
        assert_eq!(l.cell_x(3), 3. * (l.size + GAP) + GAP + SEP_W);
        assert_eq!(l.pick(0.), 0);
        assert_eq!(l.pick(l.size + 2.), 0);
        assert_eq!(l.pick(1000.), 5);
        // Unfitted cells keep their default size; narrow fields stop shrinking
        assert_eq!(Layout::new(6, None, None).size, CELL);
        assert_eq!(Layout::new(6, None, Some(120.)).size, CELL_MIN);
        assert_eq!(Layout::new(4, None, Some(600.)).size, CELL_MAX);
    }

    #[test]
    fn caret_is_solid_after_activity_then_blinks() {
        assert!(caret_visible(0));
        assert!(caret_visible(499));
        assert!(!caret_visible(500));
        assert!(caret_visible(1000));
    }
}
