//! Autonomy Dial – how much an agent may do without asking.
//!
//! An agent's card in Pink's settings language: the agent's name and scope
//! over its actions, sorted into "Unattended" and "Needs approval". The
//! level is Pink's access setting (Pink: pickers.rs `access_chip_content`
//! and `render_permission_popover`): a ghost chip with the level's glyph
//! (closed lock, shield check, open lock in the warning tone) opens the
//! "Permission mode" menu, and picking a level makes the actions glide to
//! their new group. A lock pins an action to "Needs approval" at any level.
//!
//! Open the menu with a click or Enter/Space on the chip; Up/Down move its
//! cursor, Enter picks, Escape closes.
//!
//! ```ignore
//! AutonomyDial::new("repo-agent", "Repo agent", vec![
//!     AgentAction::new("read", "Read code", Risk::Read).icon("icons/autonomy-dial/file-text.svg"),
//!     AgentAction::new("merge", "Merge to main", Risk::Risky),
//! ])
//! .level(1)
//! .on_change(|level, _, _| println!("level {level}"))
//! ```

use std::{collections::HashSet, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    Anchor, AnimationExt, AnyElement, AnyView, App, Bounds, Context, Div, ElementId, Entity,
    FocusHandle, FontWeight, Hsla, KeyDownEvent, MouseDownEvent, Pixels, Render, Role,
    SharedString, Stateful, Task, Window, anchored, canvas, deferred, div, prelude::*, px, svg,
};
use web_time::Instant;

#[allow(dead_code)]
mod package {
    include!("../../components/autonomy-dial/src/lib.rs");
}
use package as dial;
pub use package::{
    EMPTY_TEXT, GROUP_GAP, HEAD_H, HEAD_TEXT, HeadingStyle, LABEL_TEXT, MARK_GAP, MARK_SLOT,
    META_TEXT, PAD_X, ROW_H, RowStyle, empty, group, groups, heading, row, ui_rems,
};

use crate::{
    HoverFill,
    motion::{self, FADE_QUICK, MENU_OUT, MENU_RISE_DOWN, RESORT},
    pink::{self, FillTier, Pink, TextStyle, TypeStyleExt, chrome, layout},
    story,
};

pub const NAME: &str = "Autonomy Dial";
pub const DESCRIPTION: &str = "An agent's access card: a quiet level chip opens the Permission mode menu (Limited access, Approve for me, Full access) and the agent's actions glide between Unattended and Needs approval, with a lock for any action that must always ask";

const ICON_LOCK: &str = "icons/autonomy-dial/lock-keyhole-minimalistic.svg";
const ICON_UNLOCK: &str = "icons/autonomy-dial/lock-keyhole-minimalistic-unlocked.svg";
const ICON_SHIELD_CHECK: &str = "icons/autonomy-dial/shield-check.svg";

const STOPS: usize = 3;

/// The card's width.
const CARD_W: f32 = 480.;
/// The permission menu's width (Pink: pickers.rs `popover_frame(340.0, ..)`).
const MENU_W: f32 = 340.;
/// The chip's glyph (Pink: pickers.rs `CHIP_ICON`).
const CHIP_ICON: f32 = 16.;
/// Control tooltips wrap past this many characters (Pink: pickers.rs
/// `TOOLTIP_WRAP_CHARS`).
const TOOLTIP_WRAP_CHARS: usize = 40;
// The group heading, row and gap sizes are the package's (`HEAD_H`,
// `ROW_H`, `GROUP_GAP`): Pink's spaces.rs sidebar disclosure header and
// section gap, and shell.rs `compact_chat_row_height`.

type LevelHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type LockHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// What an action can break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    /// Looks, never changes anything.
    Read,
    /// Routine, reversible changes.
    Write,
    /// Hard or impossible to take back.
    Risky,
}

impl Risk {
    /// The lowest level at which this action goes ahead unattended.
    fn runs_from(self) -> usize {
        match self {
            Risk::Read => 0,
            Risk::Write => 1,
            Risk::Risky => 2,
        }
    }

    /// The row's meta word and its tone: quiet, except an irreversible
    /// action, which wears the warning tone Full access does.
    fn meta(self, t: &Pink) -> (&'static str, Hsla) {
        match self {
            Risk::Read => ("Read only", t.readable_muted(0.55)),
            Risk::Write => ("Reversible", t.readable_muted(0.55)),
            Risk::Risky => ("Irreversible", t.readable_tone(t.warning, 0.9)),
        }
    }
}

/// One thing the agent can do.
#[derive(Clone, Debug)]
pub struct AgentAction {
    pub id: SharedString,
    pub label: SharedString,
    pub risk: Risk,
    pub icon: Option<SharedString>,
    /// Always asks, whatever the level says.
    pub locked: bool,
}

impl AgentAction {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, risk: Risk) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            risk,
            icon: None,
            locked: false,
        }
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }
}

/// The three levels, cautious to unattended, with Pink's names
/// (Pink: execution_view.rs `permission_title`).
pub const LEVELS: [(&str, &str); STOPS] = [
    (
        "Limited access",
        "Reads go ahead; every change asks you first.",
    ),
    (
        "Approve for me",
        "A reviewer approves reversible changes; anything irreversible still asks you.",
    ),
    (
        "Full access",
        "Skip approval prompts; only locked actions ask.",
    ),
];

/// The level's chip glyph and tint (Pink: pickers.rs `access_chip_content`):
/// Limited is a closed lock, Approve for me a shield check, Full access an
/// open lock in the warning tone.
fn level_mark(level: usize, t: &Pink) -> (&'static str, Option<Hsla>) {
    match level {
        0 => (ICON_LOCK, None),
        1 => (ICON_SHIELD_CHECK, None),
        _ => (ICON_UNLOCK, Some(t.warning)),
    }
}

/// Whether `action` goes ahead without asking at `level`.
fn unattended(action: &AgentAction, level: usize, locked: &HashSet<SharedString>) -> bool {
    !locked.contains(&action.id) && action.risk.runs_from() <= level
}

/// The menu cursor one step up or down, wrapping (Pink: popover.rs
/// `menu_step`).
fn step(active: usize, down: bool) -> usize {
    if down {
        (active + 1) % STOPS
    } else {
        (active + STOPS - 1) % STOPS
    }
}

struct DialState {
    level: usize,
    locked: HashSet<SharedString>,
    /// Bumped whenever the groups change, to replay the empty-group fade.
    rev: u64,
    /// The chip's focus: the menu's keys are read while it holds it.
    focus: FocusHandle,
    open: bool,
    /// The cursor row while the menu is open.
    active: usize,
    closing: Option<Instant>,
    _close_task: Option<Task<()>>,
    /// Counts openings, so each entrance animation gets a fresh id.
    opened: u64,
    /// The chip's bounds: a press on it toggles instead of dismissing.
    trigger: Bounds<Pixels>,
}

impl DialState {
    fn set_level(&mut self, level: usize) -> Option<usize> {
        let level = level.min(STOPS - 1);
        if level == self.level {
            return None;
        }
        self.level = level;
        self.rev += 1;
        Some(level)
    }

    /// Pink: `toggle` opening the access picker, the cursor on the current
    /// level.
    fn open_menu(&mut self) {
        self.open = true;
        self.closing = None;
        self._close_task = None;
        self.opened += 1;
        self.active = self.level;
    }

    /// Starts the exit fade; the menu unmounts when it ends.
    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.closing = Some(Instant::now());
        let wait = MENU_OUT.total().mul_f32(motion::speed_scale()) + Duration::from_millis(16);
        self._close_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |s, cx| {
                s.closing = None;
                cx.notify();
            });
        }));
    }

    /// Pink: `pick_permission`: the level applies and the menu closes.
    fn pick(&mut self, level: usize, cx: &mut Context<Self>) -> Option<usize> {
        self.close_menu(cx);
        self.set_level(level)
    }

    fn toggle_lock(&mut self, id: &SharedString) -> bool {
        self.rev += 1;
        if self.locked.remove(id) {
            false
        } else {
            self.locked.insert(id.clone());
            true
        }
    }
}

/// An agent's access card: its level chip and menu, and its actions sorted
/// into those that run unattended and those that need approval.
#[derive(IntoElement)]
pub struct AutonomyDial {
    id: ElementId,
    agent: SharedString,
    detail: SharedString,
    hue: f32,
    actions: Vec<AgentAction>,
    level: usize,
    menu_open: bool,
    on_change: Option<LevelHandler>,
    on_lock: Option<LockHandler>,
}

impl AutonomyDial {
    pub fn new(
        id: impl Into<ElementId>,
        agent: impl Into<SharedString>,
        actions: Vec<AgentAction>,
    ) -> Self {
        Self {
            id: id.into(),
            agent: agent.into(),
            detail: SharedString::default(),
            hue: 250.,
            actions,
            level: 1,
            menu_open: false,
            on_change: None,
            on_lock: None,
        }
    }

    /// A line under the agent's name (model, scope).
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = detail.into();
        self
    }

    /// Hue (degrees) of the agent's monogram tile, on Pink's project-tint
    /// ramp.
    pub fn hue(mut self, degrees: f32) -> Self {
        self.hue = degrees;
        self
    }

    /// Initial level, 0 (Limited access) to 2 (Full access).
    pub fn level(mut self, level: usize) -> Self {
        self.level = level.min(STOPS - 1);
        self
    }

    /// Starts with the Permission mode menu open, its cursor on the level.
    pub fn menu_open(mut self, open: bool) -> Self {
        self.menu_open = open;
        self
    }

    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// A lock was toggled: `(action id, locked)`.
    pub fn on_lock(
        mut self,
        f: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_lock = Some(Rc::new(f));
        self
    }
}

fn emit(handler: &Option<LevelHandler>, level: Option<usize>, window: &mut Window, cx: &mut App) {
    if let (Some(f), Some(level)) = (handler, level) {
        f(level, window, cx);
    }
}

impl RenderOnce for AutonomyDial {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial = self.level;
        let initially_open = self.menu_open;
        let initial_locked = self
            .actions
            .iter()
            .filter(|a| a.locked)
            .map(|a| a.id.clone())
            .collect::<HashSet<_>>();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| DialState {
                level: initial,
                locked: initial_locked,
                rev: 0,
                focus: cx.focus_handle().tab_stop(true),
                open: initially_open,
                active: initial,
                closing: None,
                _close_task: None,
                opened: 0,
                trigger: Bounds::default(),
            },
        );

        // The open menu reads its keys before the focused chip does, so
        // Enter picks the cursor row instead of clicking the chip.
        let key_state = state.clone();
        let key_change = self.on_change.clone();
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if !key_state.read(cx).open || event.keystroke.modifiers.modified() {
                return;
            }
            let changed = match event.keystroke.key.as_str() {
                dir @ ("up" | "down") => {
                    key_state.update(cx, |s, cx| {
                        s.active = step(s.active, dir == "down");
                        cx.notify();
                    });
                    None
                }
                "enter" => key_state.update(cx, |s, cx| {
                    let changed = s.pick(s.active, cx);
                    cx.notify();
                    changed
                }),
                "escape" => {
                    key_state.update(cx, |s, cx| {
                        s.close_menu(cx);
                        cx.notify();
                    });
                    None
                }
                _ => return,
            };
            window.prevent_default();
            cx.stop_propagation();
            emit(&key_change, changed, window, cx);
        };

        let s = state.read(cx);
        let (level, locked, rev) = (s.level, s.locked.clone(), s.rev);
        let tint = gpui::hsla(self.hue.rem_euclid(360.) / 360., 0.62, 0.68, 1.);
        let header = div()
            .px(px(layout::ROW_PAD_X))
            .pt(px(14.))
            .pb(px(12.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(pink::monogram_tile(tint, &self.agent, 28., &t))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .type_style(TextStyle::Title3)
                            .text_color(t.text)
                            .child(self.agent.clone()),
                    )
                    .when(!self.detail.is_empty(), |column| {
                        column.child(pink::row_description(self.detail.clone(), &t))
                    }),
            )
            .child(access_chip(&key, s, &t, &state, self.on_change.clone()));

        let lanes = lanes(
            &key,
            &self.actions,
            level,
            &locked,
            rev,
            &state,
            self.on_lock.clone(),
            &t,
        );

        motion::keep_alive(window);
        pink::card(&t)
            .id(self.id.clone())
            .w(px(CARD_W))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .capture_key_down(on_key)
            .child(header)
            .child(
                div()
                    .border_t_1()
                    .border_color(t.border)
                    .px(px(8.))
                    .pt(px(6.))
                    .pb(px(8.))
                    .child(lanes),
            )
    }
}

/// The access chip and its menu layer (Pink: pickers.rs `trigger_chip` for
/// `PickerKind::Permission`, opened with `anchored_menu_below_end` since the
/// chip ends the header). Pink's composer shows the chip icon-only for room;
/// here it carries the level's title as the model chip does.
fn access_chip(
    key: &str,
    s: &DialState,
    t: &Pink,
    state: &Entity<DialState>,
    on_change: Option<LevelHandler>,
) -> Div {
    let (icon, tint) = level_mark(s.level, t);
    let (title, detail) = LEVELS[s.level];
    let toggler = state.clone();
    let measurer = state.clone();
    let chip = pink::trigger_chip(format!("{key}-access"), true, s.open, t)
        .relative()
        .flex_none()
        .track_focus(&s.focus)
        .aria_label(SharedString::from(title))
        .on_click(move |_, _, cx| {
            toggler.update(cx, |s, cx| {
                if s.open {
                    s.close_menu(cx);
                } else {
                    s.open_menu();
                }
                cx.notify();
            })
        })
        // The open menu already says everything the tooltip would.
        .when(!s.open, |el| {
            el.tooltip(move |_, cx| ControlTooltip::view(title.into(), Some(detail.into()), cx))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
        })
        .child(
            svg()
                .path(icon)
                .size(px(CHIP_ICON))
                .flex_none()
                .text_color(tint.unwrap_or(t.text_muted)),
        )
        .child(div().min_w_0().truncate().child(title))
        .child(
            canvas(
                move |bounds, _, cx| measurer.update(cx, |s, _| s.trigger = bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );

    let layer = (s.open || s.closing.is_some()).then(|| {
        let exit = (!s.open)
            .then(|| s.closing.map(motion::menu_exit_progress))
            .flatten();
        let inner = div()
            .occlude()
            .pt(px(layout::POPOVER_GAP))
            .child(permission_menu(key, s, t, state, on_change));
        let inner: AnyElement = match exit {
            // A dying menu takes no clicks (Pink: `menu_motion`).
            Some(progress) => motion::menu_out(
                ElementId::NamedInteger(format!("{key}-menu-out").into(), s.opened),
                progress,
                inner.relative().child(div().absolute().inset_0().occlude()),
            )
            .into_any_element(),
            None => motion::menu_in_from(
                ElementId::NamedInteger(format!("{key}-menu-in").into(), s.opened),
                MENU_RISE_DOWN,
                inner,
            )
            .into_any_element(),
        };
        div().absolute().bottom_0().right_0().size_0().child(
            deferred(
                anchored()
                    .anchor(Anchor::TopRight)
                    .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                    .child(inner),
            )
            .with_priority(1),
        )
    });

    div().relative().flex_none().child(chip).children(layer)
}

/// The level menu (Pink: pickers.rs `render_permission_popover` in
/// `popover_frame`): a "Permission mode" heading over one `menu_row_nav`
/// per level, each with its explanation; Full access reads in the warning
/// tone.
fn permission_menu(
    key: &str,
    s: &DialState,
    t: &Pink,
    state: &Entity<DialState>,
    on_change: Option<LevelHandler>,
) -> Div {
    let closer = state.clone();
    let rows = LEVELS.iter().enumerate().map(|(ix, (label, description))| {
        let warning = ix == STOPS - 1;
        let mover = state.clone();
        let picker = state.clone();
        let on_change = on_change.clone();
        pink::menu_row_nav(ix == s.level, ix == s.active, t)
            .id(eid(format!("{key}-level-{ix}")))
            .role(Role::MenuItemRadio)
            .on_mouse_move(move |_, _, cx| {
                mover.update(cx, |s, cx| {
                    if s.active != ix {
                        s.active = ix;
                        cx.notify();
                    }
                })
            })
            .when(warning, |row| {
                row.text_color(t.readable_tone(t.warning, 1.0))
            })
            .on_click(move |_, window, cx| {
                let changed = picker.update(cx, |s, cx| {
                    let changed = s.pick(ix, cx);
                    cx.notify();
                    changed
                });
                emit(&on_change, changed, window, cx);
            })
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().flex().items_center().justify_between().child(*label))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(if warning {
                                t.warning.opacity(0.72)
                            } else {
                                t.text_faint
                            })
                            .child(*description),
                    ),
            )
    });
    pink::popover_card(t)
        .w(px(MENU_W))
        .max_h(px(640.))
        .flex()
        .flex_col()
        .on_mouse_down_out(move |event: &MouseDownEvent, _, cx| {
            closer.update(cx, |s, cx| {
                if !s.open || s.trigger.contains(&event.position) {
                    return;
                }
                s.close_menu(cx);
                cx.notify();
            })
        })
        .child(
            div()
                .flex()
                .flex_col()
                .p(px(4.))
                .child(
                    div()
                        .px(px(10.))
                        .pt(px(8.))
                        .pb(px(6.))
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text_muted)
                        .child("Permission mode"),
                )
                .children(rows),
        )
}

/// A group heading (Pink: activity_center.rs `activity_section_heading`):
/// 28 tall, `px 8`, the name 11 semibold in `readable_faint(0.9)` and the
/// count 11 in `readable_faint(0.7)`.
fn group_heading(
    id: impl Into<String>,
    label: &'static str,
    count: usize,
    y: f32,
    t: &Pink,
) -> Stateful<Div> {
    let noun = if count == 1 { "action" } else { "actions" };
    let style = HeadingStyle {
        title: t.readable_faint(0.9),
        count: t.readable_faint(0.7),
    };
    dial::heading(&style, label, count)
        .id(eid(id))
        .absolute()
        .left_0()
        .right_0()
        .top(px(y))
        .role(Role::Heading)
        .aria_label(SharedString::from(format!("{label}, {count} {noun}")))
}

/// The two groups. Every action sits in one of them, and a level change or
/// a lock makes rows glide to their new place (Pink: `RESORT`, the sidebar's
/// list reorder). The block keeps one height while the level turns; only an
/// emptied group, which holds a quiet line instead, can change it.
#[allow(clippy::too_many_arguments)]
fn lanes(
    key: &str,
    actions: &[AgentAction],
    level: usize,
    locked: &HashSet<SharedString>,
    rev: u64,
    state: &Entity<DialState>,
    on_lock: Option<LockHandler>,
    t: &Pink,
) -> Div {
    let runs = actions
        .iter()
        .filter(|a| unattended(a, level, locked))
        .count();
    let asks = actions.len() - runs;
    let second = HEAD_H + runs.max(1) as f32 * ROW_H + GROUP_GAP;
    let second_y = motion::follow(&format!("{key}-head-b"), second, RESORT);
    let height = motion::follow(
        &format!("{key}-lanes-h"),
        second + HEAD_H + asks.max(1) as f32 * ROW_H,
        RESORT,
    );

    // Shown once the departing rows have cleared the slot.
    let empty = |ix: usize, copy: &'static str, y: f32| {
        dial::empty(t.text_muted, copy)
            .absolute()
            .left_0()
            .right_0()
            .top(px(y))
            .with_animation(
                ElementId::NamedInteger(format!("{key}-empty-{ix}").into(), rev),
                FADE_QUICK.with_delay(RESORT.duration_ms).animation(),
                |el, p| el.opacity(p),
            )
    };

    let mut slot = [0usize; 2];
    let rows = actions
        .iter()
        .map(|action| {
            let group = usize::from(!unattended(action, level, locked));
            let pos = slot[group];
            slot[group] += 1;
            let top = if group == 0 { 0. } else { second };
            let y = motion::follow(
                &format!("{key}-y-{}", action.id),
                top + HEAD_H + pos as f32 * ROW_H,
                RESORT,
            );
            action_row(
                key,
                action,
                y,
                locked.contains(&action.id),
                state,
                on_lock.clone(),
                t,
            )
        })
        .collect::<Vec<_>>();

    div()
        .relative()
        .w_full()
        .h(px(height))
        .child(group_heading(
            format!("{key}-head-0"),
            "Unattended",
            runs,
            0.,
            t,
        ))
        .child(group_heading(
            format!("{key}-head-1"),
            "Needs approval",
            asks,
            second_y,
            t,
        ))
        .when(runs == 0, |el| {
            el.child(empty(0, "Every action waits for you", HEAD_H))
        })
        .when(asks == 0, |el| {
            el.child(empty(1, "Nothing needs approval", second_y + HEAD_H))
        })
        .children(rows)
}

/// One action on Pink's sidebar leading grid: the glyph in `text_muted` in
/// its 16 slot (kept empty without one, so labels line up), the 13 label, an
/// 11 meta word, and the lock in the corner.
fn action_row(
    key: &str,
    action: &AgentAction,
    y: f32,
    is_locked: bool,
    state: &Entity<DialState>,
    on_lock: Option<LockHandler>,
    t: &Pink,
) -> Div {
    let group: SharedString = format!("{key}-row-{}", action.id).into();
    let (meta, tone) = action.risk.meta(t);
    let style = RowStyle {
        icon: t.text_muted,
        label: t.text,
        meta: tone,
    };
    dial::row(&style, action.icon.clone(), action.label.clone(), meta)
        .absolute()
        .left_0()
        .right_0()
        .top(px(y))
        .group(group.clone())
        .child(lock_button(
            key, action, is_locked, group, state, on_lock, t,
        ))
}

/// The row's lock (Pink: shell.rs the sidebar row's pin): 20 square, radius
/// 4, the quiet hover wash, a 12 px glyph. Locked, it stays in the accent;
/// otherwise it shows in `text_muted` while the row is hovered or the button
/// focused.
fn lock_button(
    key: &str,
    action: &AgentAction,
    is_locked: bool,
    group: SharedString,
    state: &Entity<DialState>,
    on_lock: Option<LockHandler>,
    t: &Pink,
) -> Stateful<Div> {
    let id = format!("{key}-lock-{}", action.id);
    let label = if is_locked {
        "Unlock"
    } else {
        "Always ask first"
    };
    let ring = t.focus_ring();
    let action_id = action.id.clone();
    let state = state.clone();
    div()
        .id(eid(id.clone()))
        .role(Role::Button)
        .aria_label(SharedString::from(format!("{label}: {}", action.label)))
        .tab_index(0)
        .flex_none()
        .size(px(chrome::MIN_HIT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_S))
        .cursor_pointer()
        .hover_wash(id, t.fill(FillTier::Tertiary))
        .when(!is_locked, |el| {
            el.opacity(0.).group_hover(group, |style| style.opacity(1.))
        })
        .focus_visible(move |style| pink::focus_ring(style, ring).opacity(1.))
        .on_key_down(pink::swallow_activation_key)
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            let now_locked = state.update(cx, |s, cx| {
                let now_locked = s.toggle_lock(&action_id);
                cx.notify();
                now_locked
            });
            if let Some(f) = on_lock.as_ref() {
                f(&action_id, now_locked, window, cx);
            }
        })
        .child(
            svg()
                .path(ICON_LOCK)
                .size(px(chrome::ICON_S))
                .flex_none()
                .text_color(if is_locked {
                    t.readable_icon(t.accent)
                } else {
                    t.text_muted
                }),
        )
}

/// The chip's tooltip: the level over its explanation (Pink: pickers.rs
/// `ControlTooltip`).
struct ControlTooltip {
    title: SharedString,
    detail: Option<SharedString>,
}

impl ControlTooltip {
    fn view(title: SharedString, detail: Option<SharedString>, cx: &mut App) -> AnyView {
        cx.new(|_| Self { title, detail }).into()
    }
}

impl Render for ControlTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let wraps = self.detail.as_ref().is_some_and(|detail| {
            detail
                .split('\n')
                .any(|line| line.chars().count() > TOOLTIP_WRAP_CHARS)
        });
        pink::tooltip_card(&t)
            .when(wraps, |card| card.w(px(layout::TOOLTIP_MAX_WIDTH)))
            .when(!wraps, |card| card.whitespace_nowrap())
            .px(px(10.))
            .py(px(7.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .text_size(px(12.))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(self.title.clone()),
            )
            .when_some(self.detail.clone(), |card, detail| {
                card.children(detail.split('\n').map(|line| {
                    div()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child(SharedString::from(line.to_string()))
                }))
            })
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    let icon = |name: &str| SharedString::from(format!("icons/autonomy-dial/{name}.svg"));
    let actions = vec![
        AgentAction::new("read", "Read code", Risk::Read).icon(icon("file-text")),
        AgentAction::new("tests", "Run tests", Risk::Read).icon(icon("terminal")),
        AgentAction::new("comment", "Comment on PRs", Risk::Write).icon(icon("chat-round-line")),
        AgentAction::new("push", "Push branches", Risk::Write).icon(icon("git-branch")),
        AgentAction::new("merge", "Merge to main", Risk::Risky).icon(icon("git-pull-request")),
        AgentAction::new("prune", "Delete branches", Risk::Risky)
            .icon(icon("trash-bin-minimalistic")),
        AgentAction::new("keys", "Rotate keys", Risk::Risky)
            .icon(icon("key-minimalistic"))
            .locked(true),
    ];
    let release = vec![
        AgentAction::new("notes", "Draft release notes", Risk::Write).icon(icon("file-text")),
        AgentAction::new("tag", "Tag the release", Risk::Write).icon(icon("git-branch")),
        AgentAction::new("publish", "Publish to the registry", Risk::Risky)
            .icon(icon("git-pull-request")),
        AgentAction::new("yank", "Yank a version", Risk::Risky)
            .icon(icon("trash-bin-minimalistic"))
            .locked(true),
    ];
    story::view(cx, move |_, _| {
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                AutonomyDial::new("autonomy-dial", "Repo agent", actions.clone())
                    .detail("lumen-4 in acme/billing-api")
                    .hue(205.)
                    .level(1)
                    .menu_open(true),
            )
            .child(
                AutonomyDial::new("autonomy-dial-release", "Release bot", release.clone())
                    .detail("lumen-4-mini in acme/billing-api")
                    .hue(150.)
                    .level(2),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_unlock_by_risk() {
        assert_eq!(Risk::Read.runs_from(), 0);
        assert_eq!(Risk::Write.runs_from(), 1);
        assert_eq!(Risk::Risky.runs_from(), 2);
    }

    #[test]
    fn a_lock_always_asks() {
        let merge = AgentAction::new("merge", "Merge to main", Risk::Risky);
        let mut locked = HashSet::new();
        assert!(!unattended(&merge, 1, &locked));
        assert!(unattended(&merge, 2, &locked));
        locked.insert(merge.id.clone());
        assert!(!unattended(&merge, 2, &locked));
    }

    #[test]
    fn menu_cursor_wraps() {
        assert_eq!(step(STOPS - 1, true), 0);
        assert_eq!(step(0, false), STOPS - 1);
        assert_eq!(step(0, true), 1);
    }
}
