//! Progress Tracker – an onboarding checklist on a Pink card: a title over
//! Pink's checklist summary ("1/5 · Next: …") and usage meter, then the steps
//! as square checkboxes.
//!
//! The header is Pink's usage limit row (`shell/usage.rs`): the title and a
//! one-line summary on the left, the share done as a Title2 figure on the
//! right, the 6 px meter underneath. The summary is the composer checklist's
//! own line (`composer/checklist.rs`), and the steps are Pink's markdown task
//! items (`markdown/render.rs`) with the dialog checkbox, as clickable rows.
//!
//! Clicking a step (or Enter / Space on a focused one) checks it off: the
//! box eases to the accent, the label settles to muted, the meter glides and
//! the summary swaps in. Up / Down / Home / End move between steps. With
//! `auto_play` the open steps tick one after another and start over, until
//! someone clicks or presses a key inside the card.
//!
//! ```ignore
//! ProgressTracker::new("onboarding", vec![
//!     ProgressStep::new("tools", "Connect your tools"),
//!     ProgressStep::new("project", "Create a project").done(true),
//! ])
//! .auto_play(true)
//! .on_change(|steps, _, _| println!("{} done", steps.iter().filter(|s| s.done).count()))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::Duration;

use gpui::{
    AnyView, App, ClickEvent, Context, Div, ElementId, FocusHandle, IntoElement, KeyDownEvent,
    MouseDownEvent, Role, SharedString, Stateful, Task, Toggled, Window, div, prelude::*, px,
    relative,
};

use crate::{
    HoverFill,
    motion::{self, EMPHASIZED, QUICK, TOOL_ROW_REVEAL},
    pink::{self, FillTier, Pink, TextStyle, TypeStyleExt, layout, loaders::label_id, radius},
    story,
};

pub const NAME: &str = "Progress Tracker";
pub const DESCRIPTION: &str = "Onboarding checklist card – a done/total summary over a quiet meter, and square checkboxes that tick steps off, one after another in auto play";

/// The card's width: Pink's dialog width.
const WIDTH: f32 = layout::DIALOG_WIDTH;

type ChangeHandler = Rc<dyn Fn(&[ProgressStep], &mut Window, &mut App)>;

/// One step of the checklist.
#[derive(Clone)]
pub struct ProgressStep {
    pub id: SharedString,
    pub label: SharedString,
    pub done: bool,
}

impl ProgressStep {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            done: false,
        }
    }

    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }
}

/// Pink: composer/checklist.rs `render_checklist` – the summary line,
/// `"{done}/{total} · {detail}"`, naming the next open step.
fn summary(steps: &[(SharedString, bool)]) -> String {
    let done = steps.iter().filter(|(_, d)| *d).count();
    let total = steps.len();
    let detail = match steps.iter().find(|(_, d)| !*d) {
        Some((label, _)) => format!("Next: {label}"),
        None if total > 0 => "Checklist complete".into(),
        None => "No active step".into(),
    };
    format!("{done}/{total} · {detail}")
}

struct State {
    steps: Vec<ProgressStep>,
    done: Vec<bool>,
    focus: Vec<FocusHandle>,
    playing: bool,
    on_change: Option<ChangeHandler>,
    _play: Option<Task<()>>,
}

impl State {
    fn new(
        steps: Vec<ProgressStep>,
        auto_play: bool,
        interval: Duration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let play = auto_play.then(|| Self::play(interval, window, cx));
        Self {
            done: steps.iter().map(|s| s.done).collect(),
            focus: steps
                .iter()
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            steps,
            playing: auto_play,
            on_change: None,
            _play: play,
        }
    }

    /// Ticks the next open step every `interval`; when all are done, waits
    /// twice as long and starts over.
    fn play(interval: Duration, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let wait = this.update(cx, |s, _| {
                    s.playing.then(|| {
                        if s.done.iter().all(|d| *d) {
                            interval * 2
                        } else {
                            interval
                        }
                    })
                });
                let Ok(Some(wait)) = wait else { break };
                cx.background_executor().timer(wait).await;
                let changed = this.update(cx, |s, cx| {
                    if !s.playing {
                        return None;
                    }
                    match s.done.iter().position(|d| !d) {
                        Some(ix) => s.done[ix] = true,
                        None => s.reset(),
                    }
                    cx.notify();
                    Some(s.report())
                });
                let Ok(Some((handler, steps))) = changed else {
                    break;
                };
                if let Some(handler) = handler {
                    let _ = cx.update(|window, cx| handler(&steps, window, cx));
                }
            }
        })
    }

    /// Back to the steps as they were passed in.
    fn reset(&mut self) {
        self.done = self.steps.iter().map(|s| s.done).collect();
    }

    fn stop(&mut self) {
        self.playing = false;
        self._play = None;
    }

    /// The change handler and the steps with their current state.
    fn report(&self) -> (Option<ChangeHandler>, Vec<ProgressStep>) {
        let steps = self
            .steps
            .iter()
            .zip(&self.done)
            .map(|(s, &done)| ProgressStep { done, ..s.clone() })
            .collect();
        (self.on_change.clone(), steps)
    }

    /// Steps passed on this render: keep the state of the ones we know.
    fn sync(&mut self, steps: &[ProgressStep], cx: &mut Context<Self>) {
        let same = steps.len() == self.steps.len()
            && steps.iter().zip(&self.steps).all(|(a, b)| a.id == b.id);
        if !same {
            let old: Vec<(SharedString, bool)> = self
                .steps
                .iter()
                .map(|s| s.id.clone())
                .zip(self.done.iter().copied())
                .collect();
            self.done = steps
                .iter()
                .map(|s| {
                    old.iter()
                        .find(|(id, _)| *id == s.id)
                        .map_or(s.done, |(_, d)| *d)
                })
                .collect();
            self.focus = steps
                .iter()
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect();
        }
        self.steps = steps.to_vec();
    }
}

#[derive(IntoElement)]
pub struct ProgressTracker {
    id: ElementId,
    title: SharedString,
    steps: Vec<ProgressStep>,
    auto_play: bool,
    interval: Duration,
    on_change: Option<ChangeHandler>,
}

impl ProgressTracker {
    pub fn new(id: impl Into<ElementId>, steps: Vec<ProgressStep>) -> Self {
        Self {
            id: id.into(),
            title: "Getting started".into(),
            steps,
            auto_play: false,
            interval: Duration::from_millis(2200),
            on_change: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Ticks the open steps one after another, then starts over – stops as
    /// soon as someone interacts.
    pub fn auto_play(mut self, auto_play: bool) -> Self {
        self.auto_play = auto_play;
        self
    }

    /// Pause between two automatic steps (default 2.2s).
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&[ProgressStep], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ProgressTracker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = format!("pt-{}", self.id);
        let (steps, auto_play, interval) = (self.steps.clone(), self.auto_play, self.interval);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| State::new(steps, auto_play, interval, window, cx),
        );
        state.update(cx, |s, cx| {
            s.sync(&self.steps, cx);
            s.on_change = self.on_change.clone();
        });

        let s = state.read(cx);
        let total = s.done.len();
        let count = s.done.iter().filter(|d| **d).count();
        let progress = if total > 0 {
            count as f32 / total as f32
        } else {
            0.
        };
        let percent = (progress * 100.).round() as u32;
        let steps: Vec<(SharedString, bool)> = s
            .steps
            .iter()
            .zip(&s.done)
            .map(|(step, &done)| (step.label.clone(), done))
            .collect();
        let line = summary(&steps);
        let rows: Vec<(ProgressStep, bool)> = s.steps.iter().cloned().zip(s.done.clone()).collect();
        let handles = s.focus.clone();

        let rows: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(ix, (step, done))| {
                let fill = motion::follow(
                    &format!("{key}-check-{}", step.id),
                    if done { 1. } else { 0. },
                    QUICK,
                );
                let toggle = {
                    let state = state.clone();
                    move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                        let (handler, steps) = state.update(cx, |s, cx| {
                            s.done[ix] = !s.done[ix];
                            cx.notify();
                            s.report()
                        });
                        if let Some(handler) = handler {
                            handler(&steps, window, cx);
                        }
                    }
                };
                let row = step_row(&key, ix, &step, done, fill, &t)
                    .track_focus(&handles[ix])
                    .on_click(toggle);
                // Pink: transcript.rs tool row reveal – rows arrive one after
                // another, rising 4 px.
                let delay = motion::tool_row_arrival_delay(ix, true).as_millis() as u64;
                motion::rise_in(("row-in", ix), TOOL_ROW_REVEAL.with_delay(delay), 4., row)
            })
            .collect();

        let header = header(
            &key,
            &self.title,
            &line,
            percent,
            motion::follow(&format!("{key}-meter"), progress, EMPHASIZED),
            &t,
        );

        // Any press or key inside stops the auto play
        let stop_mouse = {
            let state = state.clone();
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| s.stop());
            }
        };
        let stop_key = {
            let state = state.clone();
            move |_: &KeyDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| s.stop());
            }
        };
        let on_nav = {
            let handles = handles.clone();
            move |e: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let len = handles.len();
                if len == 0 || e.keystroke.modifiers.modified() {
                    return;
                }
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = match e.keystroke.key.as_str() {
                    "down" => current.map_or(0, |i| (i + 1).min(len - 1)),
                    "up" => current.map_or(len - 1, |i| i.saturating_sub(1)),
                    "home" => 0,
                    "end" => len - 1,
                    _ => return,
                };
                window.focus(&handles[next], cx);
                cx.stop_propagation();
            }
        };

        // Pink: settings/widgets.rs section card surface – r12, 1 px
        // `border`, `card_glass_bg`, never shadowed; the steps sit under a
        // rule like the card's rows, inset 4 like Pink's menus.
        let card = pink::card(&t)
            .id(self.id.clone())
            .w_full()
            .max_w(px(WIDTH))
            .overflow_hidden()
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_any_mouse_down(stop_mouse)
            .capture_key_down(stop_key)
            .on_key_down(on_nav)
            .child(header)
            .child(
                div()
                    .border_t_1()
                    .border_color(t.border)
                    .p(px(layout::CARD_INSET))
                    .flex()
                    .flex_col()
                    .gap(px(layout::MENU_GAP))
                    .children(rows),
            );

        motion::keep_alive(window);
        // Pink: motion.rs `fade_in` – new content fades in over a 4 px rise.
        motion::fade_in_rise(ElementId::NamedChild(Arc::new(self.id), "in".into()), card)
    }
}

/// Pink: shell/usage.rs limit row (a settings `card_row`, `px 16 / py 12`,
/// gap 12 / 10): the title (13/18 medium) over the checklist summary (12/16
/// `readable_muted(0.85)`), the figure in Title2 beside a 12/16 word, and
/// the meter as the row's full-width block. A new summary or figure swaps in
/// over `STATUS_SWAP` (Pink: transcript.rs status change).
fn header(key: &str, title: &SharedString, line: &str, percent: u32, fill: f32, t: &Pink) -> Div {
    let text = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .child(pink::row_title(title.clone(), t).flex_1().truncate()),
        )
        .child(
            div().flex().child(motion::status_swap(
                label_id(&format!("{key}-summary"), line),
                pink::row_description(SharedString::from(line.to_owned()), t)
                    .flex_1()
                    .truncate(),
            )),
        );

    let figure = percent.to_string();
    let value = div()
        .flex_none()
        .flex()
        .items_baseline()
        .gap(px(pink::space::S4))
        .child(motion::status_swap(
            label_id(&format!("{key}-percent"), &figure),
            div()
                .type_style(TextStyle::Title2)
                .text_color(t.text)
                .child(format!("{figure}%")),
        ))
        .child(
            div()
                .text_size(px(pink::text::FOOTNOTE))
                .line_height(px(16.))
                .text_color(t.readable_muted(0.85))
                .child("done"),
        );

    div()
        .px(px(layout::ROW_PAD_X))
        .py(px(layout::ROW_PAD_Y))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(text)
                .child(value),
        )
        .child(meter(fill, t))
}

/// Pink: shell/usage.rs `usage_meter` – a 6 px capsule on a `fill(Secondary)`
/// track, filled in Pink's neutral `text_muted`, nothing drawn below 0.5%.
/// Progress has no warning or danger band, so the fill stays neutral.
fn meter(progress: f32, t: &Pink) -> Div {
    let progress = progress.clamp(0., 1.);
    div()
        .w_full()
        .h(px(6.))
        .rounded_full()
        .bg(t.fill(FillTier::Secondary))
        .when(progress >= 0.005, |bar| {
            bar.child(
                div()
                    .h_full()
                    .w(relative(progress))
                    .rounded_full()
                    .bg(t.text_muted),
            )
        })
}

/// Pink: shell.rs dialog checkbox (the kit's `checkbox_box`): 16 px, r4,
/// `border` over `glass_hover`, checked an accent plate with a 12 px check in
/// `on_solid`. Between the two the plate eases over `QUICK`.
fn check(fill: f32, t: &Pink) -> Div {
    if fill <= 0. || fill >= 1. {
        return pink::checkbox_box(fill >= 1., t);
    }
    div()
        .flex_none()
        .size(px(16.))
        .rounded(px(radius::XS))
        .border_1()
        .border_color(motion::mix(t.border, t.accent, fill))
        .bg(motion::mix(t.glass_hover(), t.accent, fill))
        .flex()
        .items_center()
        .justify_center()
        .child(pink::line_glyph(
            pink::glyphs::CHECK,
            12.,
            t.on_solid.opacity(fill),
        ))
}

/// One step: Pink's markdown task item (markdown/render.rs – the box in a
/// line-high slot, gap 8) as a clickable row, `px 12 / py 6` so the box
/// lines up with the header, r8 inside the card's 4 px inset. Hover is
/// Pink's quiet `fill(Tertiary)` wash, focus the inset ring; an open step
/// reads in `text`, a done one settles to `text_muted` with its box.
fn step_row(
    key: &str,
    ix: usize,
    step: &ProgressStep,
    done: bool,
    fill: f32,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    div()
        .id(("step", ix))
        .role(Role::CheckBox)
        .aria_label(step.label.clone())
        .aria_toggled(if done { Toggled::True } else { Toggled::False })
        .flex()
        .items_center()
        .gap(px(pink::space::S8))
        .px(px(pink::space::S12))
        .py(px(pink::space::S6))
        .rounded(px(radius::inner(radius::SECTION_CARD, layout::CARD_INSET)))
        .cursor_pointer()
        .hover_wash(
            format!("{key}-step-{}", step.id),
            t.fill(FillTier::Tertiary),
        )
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .child(
            div()
                .flex_none()
                .h(px(18.))
                .flex()
                .items_center()
                .child(check(fill, t)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(pink::text::BODY))
                .line_height(px(18.))
                .text_color(motion::mix(t.text, t.text_muted, fill))
                .child(step.label.clone()),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .flex()
            .items_start()
            .gap(px(pink::space::S24))
            .child(
                div().w(px(WIDTH)).child(
                    ProgressTracker::new(
                        "progress-tracker",
                        vec![
                            ProgressStep::new("tools", "Connect your tools"),
                            ProgressStep::new("notify", "Turn on notifications"),
                            ProgressStep::new("updates", "Catch up on blockers"),
                            ProgressStep::new("priorities", "Summarize priorities"),
                            ProgressStep::new("project", "Create a project").done(true),
                        ],
                    )
                    .auto_play(true),
                ),
            )
            .child(
                div().w(px(WIDTH)).child(
                    ProgressTracker::new(
                        "release",
                        vec![
                            ProgressStep::new("freeze", "Freeze the release branch").done(true),
                            ProgressStep::new("dry-run", "Run the migration dry run").done(true),
                            ProgressStep::new("changelog", "Write the changelog").done(true),
                            ProgressStep::new("publish", "Tag and publish the builds"),
                        ],
                    )
                    .title("Release 2.4"),
                ),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(done: &[bool]) -> Vec<(SharedString, bool)> {
        done.iter()
            .enumerate()
            .map(|(ix, &d)| (SharedString::from(format!("Step {}", ix + 1)), d))
            .collect()
    }

    #[test]
    fn summary_names_the_next_open_step() {
        assert_eq!(summary(&steps(&[true, false, false])), "1/3 · Next: Step 2");
        assert_eq!(summary(&steps(&[false, true])), "1/2 · Next: Step 1");
    }

    #[test]
    fn summary_reads_complete_when_every_step_is_done() {
        assert_eq!(summary(&steps(&[true, true])), "2/2 · Checklist complete");
        assert_eq!(summary(&steps(&[])), "0/0 · No active step");
    }
}
