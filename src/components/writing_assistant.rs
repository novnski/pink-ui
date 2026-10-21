//! Writing Assistant – an original component in luna style.
//!
//! A rewrite companion for a piece of selected text. The reader picks a
//! direction ("Friendlier", "Shorter", …), types an instruction or dictates
//! one, and the rewrite streams into a panel lit from below by a slowly
//! drifting accent glow that brightens while it writes. Every rewrite is
//! kept as a version to step through, and the chosen one can replace the
//! selection (which flashes as it updates) or be copied. While dictating, the
//! instruction field turns into a live level meter with a timer and the
//! words as they are heard.
//!
//! The component owns its input and view state; the caller owns the text:
//!
//! ```ignore
//! WritingAssistant::new("rewrite", draft)
//!     .suggestions(vec!["Friendlier".into(), "Shorter".into()])
//!     .versions(versions)                       // grows as rewrites stream in
//!     .on_request(|instruction, _, cx| { /* start a rewrite */ })
//!     .on_apply(|text, _, cx| { /* replace the selection */ })
//! ```

use std::{
    f32::consts::TAU,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnyElement, AnyView, App, Bounds, ClipboardItem, Context, ElementId, Entity,
    FontWeight, Hsla, IntoElement, SharedString, Subscription, Task, Transformation, Window,
    canvas, div, fill, linear_color_stop, linear_gradient, percentage, point, prelude::*, px, size,
    svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, oklch, radius, space, text},
    motion::{self, AnimationExt, EASE_IN_OUT, EASE_OUT, GLIDE, MotionSpec, ROW_IN},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Writing Assistant";
pub const DESCRIPTION: &str = "A rewrite companion for selected text: pick a direction, type or dictate an instruction, and watch the new version stream in over a drifting glow, then step through versions and replace the selection";

const ICON_PEN: &str = "icons/writing-assistant/pen.svg";
const ICON_SPARK: &str = "icons/writing-assistant/spark.svg";
const ICON_MIC: &str = "icons/writing-assistant/mic.svg";
const ICON_STOP: &str = "icons/writing-assistant/stop.svg";
const ICON_SEND: &str = "icons/writing-assistant/send.svg";
const ICON_CLOSE: &str = "icons/writing-assistant/close.svg";
const ICON_PREV: &str = "icons/writing-assistant/prev.svg";
const ICON_NEXT: &str = "icons/writing-assistant/next.svg";
const ICON_RETRY: &str = "icons/writing-assistant/retry.svg";
const ICON_COPY: &str = "icons/writing-assistant/copy.svg";
const ICON_CHECK: &str = "icons/writing-assistant/check.svg";
const ICON_REPLACE: &str = "icons/writing-assistant/replace.svg";
const ICON_RING: &str = "icons/writing-assistant/ring.svg";
const ICON_ARC: &str = "icons/writing-assistant/arc.svg";

const BODY_SIZE: f32 = text::CALLOUT;
const BODY_LINE: f32 = 22.;
const WORD_GAP: f32 = BODY_SIZE * 0.25;

type TextHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One rewrite of the selection.
#[derive(Clone)]
pub struct Version {
    /// What was asked ("Friendlier", or the typed instruction).
    pub label: SharedString,
    pub text: SharedString,
    /// Words shown so far while it streams; `None` once complete.
    pub reveal: Option<usize>,
}

impl Version {
    pub fn new(label: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
            reveal: None,
        }
    }

    /// Still streaming: only the first `words` words have arrived.
    pub fn streaming(mut self, words: usize) -> Self {
        self.reveal = Some(words);
        self
    }

    fn writing(&self) -> bool {
        self.reveal
            .is_some_and(|n| n < self.text.split_whitespace().count())
    }
}

struct State {
    input: Entity<TextInput>,
    has_text: bool,
    /// Index of the version on screen; `None` follows the newest.
    viewing: Option<usize>,
    seen_versions: usize,
    copied: bool,
    listen_started: Option<Instant>,
    on_request: Option<TextHandler>,
    _reset: Option<Task<()>>,
    _input_events: Subscription,
}

impl State {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Tell it what to change…"));
        let sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut State, input, ev, window, cx| match ev {
                InputEvent::Change(text) => {
                    this.has_text = !text.trim().is_empty();
                    cx.notify();
                }
                InputEvent::Submit(text) => {
                    let text = text.trim().to_string();
                    if text.is_empty() {
                        return;
                    }
                    input.update(cx, |i, cx| i.clear(cx));
                    this.has_text = false;
                    if let Some(f) = this.on_request.clone() {
                        f(text.into(), window, cx);
                    }
                    cx.notify();
                }
                _ => {}
            },
        );
        State {
            input,
            has_text: false,
            viewing: None,
            seen_versions: 0,
            copied: false,
            listen_started: None,
            on_request: None,
            _reset: None,
            _input_events: sub,
        }
    }
}

#[derive(IntoElement)]
pub struct WritingAssistant {
    id: ElementId,
    selection: SharedString,
    context: Option<SharedString>,
    suggestions: Vec<SharedString>,
    versions: Vec<Version>,
    listening: Option<SharedString>,
    width: f32,
    on_request: Option<TextHandler>,
    on_apply: Option<TextHandler>,
    on_listen: Option<ToggleHandler>,
    on_close: Option<Handler>,
}

impl WritingAssistant {
    pub fn new(id: impl Into<ElementId>, selection: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            selection: selection.into(),
            context: None,
            suggestions: Vec::new(),
            versions: Vec::new(),
            listening: None,
            width: 580.,
            on_request: None,
            on_apply: None,
            on_listen: None,
            on_close: None,
        }
    }

    /// Where the selection comes from ("Release email · draft").
    pub fn context(mut self, context: impl Into<SharedString>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn suggestions(mut self, suggestions: Vec<SharedString>) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn versions(mut self, versions: Vec<Version>) -> Self {
        self.versions = versions;
        self
    }

    /// Dictation in progress, with the words heard so far.
    pub fn listening(mut self, transcript: Option<SharedString>) -> Self {
        self.listening = transcript;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// A suggestion, a typed instruction or a finished dictation.
    pub fn on_request(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_request = Some(Rc::new(handler));
        self
    }

    /// "Replace selection" with the version on screen.
    pub fn on_apply(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_apply = Some(Rc::new(handler));
        self
    }

    /// The microphone: `true` to start listening, `false` to stop. Stopping
    /// with words heard also sends them through `on_request`.
    pub fn on_listen(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_listen = Some(Rc::new(handler));
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla, reduced: bool) -> AnyElement {
    let arc = svg().path(ICON_ARC).size(px(size)).text_color(color);
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(
            svg()
                .path(ICON_RING)
                .absolute()
                .size(px(size))
                .text_color(color.opacity(0.22)),
        )
        .child(if reduced {
            arc.into_any_element()
        } else {
            arc.with_animation(
                id,
                Animation::new(Duration::from_millis(850)).repeat(),
                |s, d| s.with_transformation(Transformation::rotate(percentage(d))),
            )
            .into_any_element()
        })
        .into_any_element()
}

/// A small square icon button.
fn tool(id: impl Into<SharedString>, icon: &'static str, t: &Luna) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let key = format!("wa-tool-{id}");
    div()
        .id(id)
        .flex_none()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .cursor_pointer()
        .hover_fill(
            key.clone(),
            t.fill(FillTier::Tertiary).opacity(0.),
            t.fill(FillTier::Tertiary),
        )
        .child(
            svg()
                .path(icon)
                .size(px(15.))
                .text_color(motion::hover_blend(&key, t.text_faint, t.text)),
        )
}

/// The glow's second hue: a soft magenta beside luna's indigo.
fn glow_hue(t: &Luna) -> Hsla {
    if t.dark {
        oklch(0.66, 0.17, 330.)
    } else {
        oklch(0.64, 0.2, 330.)
    }
}

/// Two gradient washes rising from the bottom edge, their angles drifting
/// against each other. `strength` scales them; `phase` (0..1) drives the drift.
fn glow(t: &Luna, strength: f32, phase: f32) -> impl IntoElement {
    let a = 180. + 28. * (phase * TAU).sin();
    let b = 180. - 32. * (phase * TAU + 1.3).cos();
    let (alpha_a, alpha_b) = if t.dark { (0.30, 0.20) } else { (0.16, 0.12) };
    let clear = t.surface_card.opacity(0.);
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded(px(radius::LG - 1.))
                .bg(linear_gradient(
                    a,
                    linear_color_stop(clear, 0.25),
                    linear_color_stop(t.accent.opacity(alpha_a * strength), 1.),
                )),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded(px(radius::LG - 1.))
                .bg(linear_gradient(
                    b,
                    linear_color_stop(clear, 0.45),
                    linear_color_stop(glow_hue(t).opacity(alpha_b * strength), 1.),
                )),
        )
}

/// A live level meter: bars swaying on a few sines, as if following a voice.
fn level_meter(started: Instant, color: Hsla) -> impl IntoElement {
    canvas(
        move |_, _, _| started.elapsed().as_secs_f32(),
        move |bounds, time, window, _| {
            let bars = 22;
            let bar_w = 2.;
            let gap = 2.;
            let mid = f32::from(bounds.size.height) / 2.;
            for i in 0..bars {
                let x = f32::from(bounds.origin.x) + i as f32 * (bar_w + gap);
                let f = i as f32;
                let level =
                    0.5 + 0.5 * (time * 7.1 + f * 0.9).sin() * (time * 2.3 + f * 0.37).cos();
                let envelope = 0.45 + 0.55 * (time * 1.7 + f * 0.21).sin().abs();
                let h = (3. + (f32::from(bounds.size.height) - 3.) * level * envelope).max(3.);
                let y = f32::from(bounds.origin.y) + mid - h / 2.;
                window.paint_quad(
                    fill(
                        Bounds::new(point(px(x), px(y)), size(px(bar_w), px(h))),
                        color,
                    )
                    .corner_radii(px(1.)),
                );
            }
        },
    )
    .w(px(22. * 4. - 2.))
    .h(px(18.))
    .flex_none()
}

impl RenderOnce for WritingAssistant {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let reduced = motion::reduced_motion(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            State::new,
        );

        let listening = self.listening.is_some();
        let version_count = self.versions.len();
        let on_request = self.on_request.clone();
        let input_colors = (t.text_faint, t.accent, t.selection);
        state.update(cx, |s, cx| {
            s.on_request = on_request.clone();
            // A new version takes the stage.
            if version_count != s.seen_versions {
                s.seen_versions = version_count;
                s.viewing = None;
            }
            match (listening, s.listen_started) {
                (true, None) => s.listen_started = Some(Instant::now()),
                (false, Some(_)) => s.listen_started = None,
                _ => {}
            }
            s.input.update(cx, |i, _| {
                i.set_colors(
                    Some(input_colors.0),
                    Some(input_colors.1),
                    Some(input_colors.2),
                )
            });
        });
        let s = state.read(cx);
        let input = s.input.clone();
        let has_text = s.has_text;
        let copied = s.copied;
        let listen_started = s.listen_started;
        let shown_ix = s
            .viewing
            .filter(|ix| *ix < version_count)
            .or(version_count.checked_sub(1));
        let shown = shown_ix.map(|ix| self.versions[ix].clone());
        let writing = shown.as_ref().is_some_and(|v| v.writing());

        // ---- header ---------------------------------------------------------
        let header = div()
            .px(px(space::S16))
            .h(px(52.))
            .flex()
            .items_center()
            .gap(px(space::S10))
            .child(
                div()
                    .flex_none()
                    .size(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::MD))
                    .bg(t.accent_wash)
                    .child(svg().path(ICON_PEN).size(px(16.)).text_color(if t.dark {
                        t.text
                    } else {
                        t.accent
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child("Rewrite"),
                    )
                    .when_some(self.context.clone(), |d, c| {
                        d.child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(c),
                        )
                    }),
            )
            .when_some(self.on_close.clone(), |d, f| {
                d.child(
                    tool(format!("{key}-close"), ICON_CLOSE, &t)
                        .on_click(move |_, window, cx| f(window, cx)),
                )
            });

        // ---- the selection --------------------------------------------------
        let flash_id = ElementId::NamedInteger(
            "wa-flash".into(),
            self.selection.len() as u64 * 31
                + self
                    .selection
                    .chars()
                    .take(24)
                    .map(|c| c as u64)
                    .sum::<u64>(),
        );
        let wash = t.accent.opacity(if t.dark { 0.28 } else { 0.14 });
        let base_bg = t.fill(FillTier::Quaternary);
        let selection = div()
            .mx(px(space::S16))
            .child(
                div()
                    .text_size(px(text::CAPTION))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text_faint)
                    .mb(px(space::S6))
                    .child("Selected text"),
            )
            .child(
                div()
                    .id("wa-selection")
                    .rounded(px(radius::MD))
                    .border_1()
                    .border_color(t.border)
                    .px(px(space::S12))
                    .py(px(space::S10))
                    .text_size(px(text::BODY))
                    .line_height(px(20.))
                    .text_color(t.text_muted)
                    .line_clamp(3)
                    .child(self.selection.clone())
                    .with_animation(
                        flash_id,
                        MotionSpec::new(1200, EASE_OUT).animation(),
                        move |el, d| el.bg(motion::mix(wash, base_bg, d)),
                    ),
            );

        // ---- suggestions ------------------------------------------------------
        let active_label = shown.as_ref().map(|v| v.label.clone());
        let chips = self.suggestions.iter().enumerate().map(|(i, label)| {
            let selected = active_label.as_ref() == Some(label);
            let on_request = self.on_request.clone();
            let label2 = label.clone();
            luna::chip(format!("{key}-chip-{i}"), selected, ControlSize::Md, &t)
                .when_some(on_request, |c, f| {
                    c.on_click(move |_, window, cx| f(label2.clone(), window, cx))
                })
                .child(label.clone())
        });
        let suggestions = div()
            .mx(px(space::S16))
            .flex()
            .flex_wrap()
            .gap(px(space::S6))
            .children(chips);

        // ---- the rewrite panel ------------------------------------------------
        let strength = motion::follow(
            &format!("{key}-glow"),
            if writing {
                1.
            } else if shown.is_some() {
                0.55
            } else {
                0.35
            },
            MotionSpec::new(900, EASE_IN_OUT),
        );
        let glow_layer: AnyElement = if reduced {
            glow(&t, strength, 0.15).into_any_element()
        } else {
            let tt = t.clone();
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .with_animation(
                    "wa-glow",
                    Animation::new(Duration::from_millis(9000)).repeat(),
                    move |el, d| el.child(glow(&tt, strength, d)),
                )
                .into_any_element()
        };

        let panel_body: AnyElement = match &shown {
            None => div()
                .flex()
                .flex_col()
                .gap(px(space::S4))
                .py(px(space::S10))
                .child(
                    div()
                        .text_size(px(text::CALLOUT))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child("How should it read?"),
                )
                .child(
                    div()
                        .text_size(px(text::BODY))
                        .text_color(t.text_muted)
                        .child("Pick a direction above, or tell me below. Your text stays as it is until you replace it."),
                )
                .into_any_element(),
            Some(version) => {
                let ix = shown_ix.unwrap_or(0);
                let all: Vec<&str> = version.text.split_whitespace().collect();
                let limit = version.reveal.unwrap_or(all.len()).min(all.len());
                let streaming = version.writing();
                let mut words: Vec<AnyElement> = all[..limit]
                    .iter()
                    .enumerate()
                    .map(|(n, w)| {
                        div()
                            .flex_none()
                            .pr(px(WORD_GAP))
                            .child(SharedString::from(w.to_string()))
                            .with_animation(
                                ElementId::NamedInteger(format!("wa-w-{ix}").into(), n as u64),
                                MotionSpec::new(220, EASE_OUT).animation(),
                                |el, d| el.opacity(0.15 + 0.85 * d).top(px(2. * (1. - d))),
                            )
                            .into_any_element()
                    })
                    .collect();
                if streaming {
                    let caret = div()
                        .flex_none()
                        .w(px(2.))
                        .h(px(15.))
                        .rounded(px(1.))
                        .bg(t.accent);
                    words.push(if reduced {
                        caret.into_any_element()
                    } else {
                        caret
                            .with_animation(
                                "wa-caret",
                                Animation::new(Duration::from_millis(1000)).repeat(),
                                |el, d| el.opacity(0.6 + 0.4 * (d * TAU).cos()),
                            )
                            .into_any_element()
                    });
                }

                let state_prev = state.clone();
                let state_next = state.clone();
                let nav = (version_count > 1).then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .child(
                            tool(format!("{key}-prev"), ICON_PREV, &t)
                                .size(px(22.))
                                .when(ix == 0, |d| d.opacity(0.4))
                                .on_click(move |_, _, cx| {
                                    state_prev.update(cx, |s, cx| {
                                        s.viewing = Some(ix.saturating_sub(1));
                                        cx.notify();
                                    })
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(text::CAPTION))
                                .font_family(luna::MONO)
                                .text_color(t.text_faint)
                                .child(format!("{}/{}", ix + 1, version_count)),
                        )
                        .child(
                            tool(format!("{key}-next"), ICON_NEXT, &t)
                                .size(px(22.))
                                .when(ix + 1 >= version_count, |d| d.opacity(0.4))
                                .on_click(move |_, _, cx| {
                                    state_next.update(cx, |s, cx| {
                                        s.viewing = Some((ix + 1).min(version_count - 1));
                                        cx.notify();
                                    })
                                }),
                        )
                });

                let actions = (!streaming).then(|| {
                    let on_apply = self.on_apply.clone();
                    let on_request = self.on_request.clone();
                    let text_apply = version.text.clone();
                    let text_copy = version.text.clone();
                    let label = version.label.clone();
                    let copy_state = state.clone();
                    let delta = all.len() as i64
                        - self.selection.split_whitespace().count() as i64;
                    let buttons: Vec<AnyElement> = vec![
                        luna::button(format!("{key}-apply"), ButtonKind::Primary, ControlSize::Md, &t)
                            .child(luna::glyph(ICON_REPLACE, ControlSize::Md).text_color(t.on_solid))
                            .child("Replace selection")
                            .when_some(on_apply, |b, f| {
                                b.on_click(move |_, window, cx| f(text_apply.clone(), window, cx))
                            })
                            .into_any_element(),
                        luna::button(format!("{key}-copy"), ButtonKind::Secondary, ControlSize::Md, &t)
                            .child(
                                luna::glyph(if copied { ICON_CHECK } else { ICON_COPY }, ControlSize::Md)
                                    .text_color(if copied { t.success } else { t.text_muted }),
                            )
                            .child(if copied { "Copied" } else { "Copy" })
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text_copy.to_string()));
                                let entity = copy_state.clone();
                                copy_state.update(cx, |s, cx| {
                                    s.copied = true;
                                    cx.notify();
                                    s._reset = Some(cx.spawn(async move |_, cx| {
                                        cx.background_executor().timer(Duration::from_millis(1400)).await;
                                        let _ = entity.update(cx, |s, cx| {
                                            s.copied = false;
                                            cx.notify();
                                        });
                                    }));
                                });
                            })
                            .into_any_element(),
                        tool(format!("{key}-retry"), ICON_RETRY, &t)
                            .when_some(on_request, |b, f| {
                                b.on_click(move |_, window, cx| f(label.clone(), window, cx))
                            })
                            .into_any_element(),
                    ];
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .mt(px(space::S12))
                        .children(buttons.into_iter().enumerate().map(|(i, b)| {
                            motion::rise_in(
                                ElementId::NamedInteger(format!("wa-act-{ix}").into(), i as u64),
                                ROW_IN.with_delay(50 * i as u64),
                                4.,
                                div().child(b),
                            )
                        }))
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_faint)
                                .child(match delta {
                                    0 => "Same length".to_string(),
                                    d if d < 0 => format!("{} words shorter", -d),
                                    d => format!("{d} words longer"),
                                }),
                        )
                });

                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S6))
                            .h(px(22.))
                            .mb(px(space::S6))
                            .child(if streaming {
                                spinner("wa-spin", 13., t.accent, reduced)
                            } else {
                                svg()
                                    .path(ICON_SPARK)
                                    .size(px(14.))
                                    .text_color(t.accent)
                                    .into_any_element()
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(text::FOOTNOTE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text_muted)
                                    .child(if streaming {
                                        format!("{} · writing", version.label)
                                    } else {
                                        version.label.to_string()
                                    }),
                            )
                            .children(nav),
                    )
                    .child(
                        div()
                            .id(ElementId::NamedInteger("wa-text".into(), ix as u64))
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .text_size(px(BODY_SIZE))
                            .line_height(px(BODY_LINE))
                            .text_color(t.text)
                            .children(words),
                    )
                    .children(actions)
                    .into_any_element()
            }
        };
        let panel = div()
            .mx(px(space::S16))
            .relative()
            .overflow_hidden()
            .rounded(px(radius::LG))
            .border_1()
            .border_color(t.border)
            .bg(t.surface_card)
            .child(glow_layer)
            .child(
                div()
                    .relative()
                    .px(px(space::S16))
                    .pt(px(space::S12))
                    .pb(px(space::S16))
                    .min_h(px(132.))
                    .child(panel_body),
            );

        // ---- instruction / dictation -----------------------------------------
        let on_listen = self.on_listen.clone();
        let transcript = self.listening.clone();
        let on_request_mic = self.on_request.clone();
        let mic = div()
            .id("wa-mic")
            .flex_none()
            .size(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .map(|d| {
                if listening {
                    d.bg(t.danger.opacity(if t.dark { 0.22 } else { 0.12 }))
                        .child(svg().path(ICON_STOP).size(px(13.)).text_color(t.danger))
                } else {
                    let mk = format!("{key}-mic");
                    d.hover_fill(
                        mk.clone(),
                        t.fill(FillTier::Tertiary).opacity(0.),
                        t.fill(FillTier::Tertiary),
                    )
                    .child(
                        svg()
                            .path(ICON_MIC)
                            .size(px(16.))
                            .text_color(motion::hover_blend(&mk, t.text_muted, t.text)),
                    )
                }
            })
            .on_click(move |_, window, cx| {
                if let Some(f) = &on_listen {
                    f(!listening, window, cx);
                }
                if listening
                    && let (Some(words), Some(f)) = (&transcript, &on_request_mic)
                    && !words.trim().is_empty()
                {
                    f(words.clone(), window, cx);
                }
            });

        let send_ready = has_text && !listening;
        let send_input = input.clone();
        let send = div()
            .id("wa-send")
            .flex_none()
            .size(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(motion::mix(
                t.fill(FillTier::Secondary),
                t.accent_strong,
                motion::follow(
                    &format!("{key}-send"),
                    if send_ready { 1. } else { 0. },
                    GLIDE,
                ),
            ))
            .when(send_ready, |d| d.cursor_pointer())
            .on_click({
                let state = state.clone();
                move |_, window, cx| {
                    let text = send_input.read(cx).text();
                    if text.trim().is_empty() {
                        return;
                    }
                    send_input.update(cx, |i, cx| i.clear(cx));
                    let f = state.update(cx, |s, _| {
                        s.has_text = false;
                        s.on_request.clone()
                    });
                    if let Some(f) = f {
                        f(text.trim().to_string().into(), window, cx);
                    }
                }
            })
            .child(
                svg()
                    .path(ICON_SEND)
                    .size(px(15.))
                    .text_color(if send_ready {
                        t.on_accent
                    } else {
                        t.text_faint
                    }),
            );

        let field: AnyElement = if let (true, Some(started)) = (listening, listen_started) {
            window.request_animation_frame();
            let secs = started.elapsed().as_secs();
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(space::S10))
                .child(
                    div()
                        .flex_none()
                        .size(px(8.))
                        .rounded_full()
                        .bg(t.danger)
                        .with_animation(
                            "wa-rec",
                            Animation::new(Duration::from_millis(1200)).repeat(),
                            |el, d| el.opacity(0.55 + 0.45 * (d * TAU).cos()),
                        ),
                )
                .child(level_meter(started, t.danger.opacity(0.8)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(text::BODY))
                        .text_color(t.text_muted)
                        .child(match &self.listening {
                            Some(words) if !words.is_empty() => words.clone(),
                            _ => "Listening…".into(),
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(text::FOOTNOTE))
                        .font_family(luna::MONO)
                        .text_color(t.text_faint)
                        .child(format!("{}:{:02}", secs / 60, secs % 60)),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(text::BODY))
                .line_height(px(20.))
                .text_color(t.text)
                .child(input.clone())
                .into_any_element()
        };

        let composer = div()
            .mt(px(space::S4))
            .border_t_1()
            .border_color(t.border)
            .h(px(52.))
            .pl(px(space::S16))
            .pr(px(space::S10))
            .flex()
            .items_center()
            .gap(px(space::S6))
            .child(field)
            .child(mic)
            .child(send);

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .font_family(luna::SANS)
            .child(
                luna::card(&t)
                    .flex()
                    .flex_col()
                    .gap(px(space::S12))
                    .child(header)
                    .child(selection)
                    .child(suggestions)
                    .child(panel)
                    .child(composer),
            )
    }
}

// ---------------------------------------------------------------------------
// Story: polishing a release announcement.
// ---------------------------------------------------------------------------

const DRAFT: &str = "hey all, the new release is out and it has a lot of stuff in it like faster sync and the new sharing settings. please update when you get a chance and let us know if anything breaks";

const SUGGESTIONS: [&str; 4] = ["Friendlier", "Shorter", "Clearer", "Fix grammar"];

fn rewrite_for(label: &str) -> &'static str {
    match label {
        "Friendlier" => {
            "Hi everyone! Version 3.2 is out today, with faster sync and the new sharing settings you asked for. Update whenever it suits you, and tell us if anything feels off."
        }
        "Shorter" => {
            "Version 3.2 is out: faster sync and new sharing settings. Please update, and tell us if anything breaks."
        }
        "Clearer" => {
            "Version 3.2 is now available. Sync is faster, and all sharing settings now live in one place. Please update when you can and report any problems in #support."
        }
        "Fix grammar" => {
            "Hey all, the new release is out, and it includes a lot: faster sync and the new sharing settings. Please update when you get a chance, and let us know if anything breaks."
        }
        _ => {
            "Big news: version 3.2 just landed! Sync is faster and sharing settings are all new. Update today, then join Thursday's walkthrough webinar to see it in action."
        }
    }
}

const DICTATION: &str = "make it sound more excited and mention the webinar on Thursday";

const WORDS_PER_SEC: f32 = 16.;

struct Demo {
    selection: SharedString,
    /// (label, text, started)
    versions: Vec<(SharedString, &'static str, Instant)>,
    listening: Option<Instant>,
    _ticker: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let mounted = Instant::now();
        let ticker = cx.spawn(async move |this, cx| {
            let mut kicked_off = false;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let alive = this.update(cx, |demo, cx| {
                    // Open with a first rewrite so the panel has something to show.
                    if !kicked_off && mounted.elapsed() > Duration::from_millis(900) {
                        kicked_off = true;
                        if demo.versions.is_empty() {
                            demo.request("Friendlier".into());
                        }
                    }
                    // Dictation "hears" a word every 240ms, then sends itself.
                    if let Some(started) = demo.listening {
                        let total = DICTATION.split(' ').count() as f32;
                        if started.elapsed().as_secs_f32() > total * 0.24 + 0.9 {
                            demo.listening = None;
                            demo.request(DICTATION.into());
                        }
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Demo {
            selection: DRAFT.into(),
            versions: Vec::new(),
            listening: None,
            _ticker: ticker,
        }
    }

    fn request(&mut self, label: SharedString) {
        let text = rewrite_for(&label);
        let label = if SUGGESTIONS.contains(&label.as_ref()) {
            label
        } else {
            let mut l = label.to_string();
            if let Some(c) = l.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            l.into()
        };
        self.versions.push((label, text, Instant::now()));
    }

    fn transcript(&self) -> Option<SharedString> {
        self.listening.map(|started| {
            let n = (started.elapsed().as_secs_f32() / 0.24) as usize;
            DICTATION
                .split(' ')
                .take(n)
                .collect::<Vec<_>>()
                .join(" ")
                .into()
        })
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("writing-assistant-demo", cx, |_, cx| Demo::new(cx));
        let d = demo.read(cx);
        let versions = d
            .versions
            .iter()
            .map(|(label, text, started)| {
                let n = (started.elapsed().as_secs_f32() * WORDS_PER_SEC) as usize;
                let v = Version::new(label.clone(), *text);
                if n < text.split_whitespace().count() {
                    v.streaming(n)
                } else {
                    v
                }
            })
            .collect();
        let (on_req, on_apply, on_listen) = (demo.clone(), demo.clone(), demo.clone());
        WritingAssistant::new("writing-assistant", d.selection.clone())
            .context("Release announcement · #general draft")
            .suggestions(SUGGESTIONS.iter().map(|s| SharedString::from(*s)).collect())
            .versions(versions)
            .listening(d.transcript())
            .on_request(move |label, _, cx| {
                on_req.update(cx, |d, cx| {
                    d.request(label);
                    cx.notify();
                })
            })
            .on_apply(move |text, _, cx| {
                on_apply.update(cx, |d, cx| {
                    d.selection = text;
                    cx.notify();
                })
            })
            .on_listen(move |on, _, cx| {
                on_listen.update(cx, |d, cx| {
                    d.listening = on.then(Instant::now);
                    cx.notify();
                })
            })
    })
}
