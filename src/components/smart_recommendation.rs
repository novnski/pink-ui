//! Smart Recommendation – one suggested next step and its reason, drawn the
//! way Pink's Git card recommends its next step (Pink
//! `changes/git_actions.rs`).
//!
//! Two parts are Pink's own, copied as they are:
//!
//! - the next-step line (Pink `render_next_git_action`): the step's label in
//!   medium `text` beside its reason in `text_dim`, 11 px, gap 6;
//! - the rule that the accent belongs to the button that performs the step
//!   (Pink `git_step_button`): Accept wears the Git card's accent plate while
//!   the recommendation is the shown option, and turns back into the plain
//!   card button once an alternative is picked. Both are the Git card's 26 px
//!   buttons (Pink `git_button` / `git_primary_button`), drawn by the
//!   `pink-git-card` package PINK pins.
//!
//! The rest is Pink's language around them: the card is the settings card
//! surface with the Git card's insets; its header folds the line away on the
//! changes pane's file-header recipe (Pink `changes.rs render_file_header`);
//! the confidence signal is Pink's subagent status badge (a 6 px dot and an
//! 11 px medium word); Alternatives opens a keyboard menu above its button
//! (`popover_card` + `menu_row_nav`, Escape, a click outside or a pick closes
//! it); and Accept swaps the buttons for one green glyph and word with Undo.
//!
//! ```ignore
//! SmartRecommendation::new("sync", vec![
//!     Recommendation::new("incremental", "Incremental sync", Signal::Strong)
//!         .reason("Only **4%** of rows changed since yesterday."),
//!     Recommendation::new("full", "Full refresh", Signal::Weak),
//! ])
//! .title("Nightly sync")
//! .on_accept(|option, _, _| println!("{}", option.label))
//! ```

use std::{f32::consts::FRAC_PI_2, ops::Range, rc::Rc, sync::Arc};

use gpui::{
    Anchor, AnyView, App, ClickEvent, Div, ElementId, Entity, FocusHandle, Font, FontWeight, Hsla,
    IntoElement, KeyDownEvent, MouseMoveEvent, Role, SharedString, Stateful, StyledText, TextRun,
    Transformation, Window, anchored, canvas, deferred, div, font, prelude::*, px, radians, svg,
};
use web_time::Instant;

use crate::components::git_card as git;

use crate::{
    HoverFill,
    motion::{self, FOLD_TWEEN_WINDOW, MENU_RISE_UP, TOOL_BODY_RISE},
    pink::{self, Pink, TextStyle, TypeStyleExt, icon_size, layout, radius, space},
    story,
};

pub const NAME: &str = "Smart Recommendation";
pub const DESCRIPTION: &str = "A suggested next step with its reason and a confidence signal: alternatives to switch to, the accent on the button that accepts it, and undo";

/// The header strip: Pink's file-header height (`surface_chrome::HEADER_HEIGHT`,
/// the 38 pt titlebar).
const HEAD_HEIGHT: f32 = layout::TITLEBAR_HEIGHT;
/// The Git card's buttons' height (`pink-git-card` `BUTTON_HEIGHT`), which
/// the accepted line keeps.
const GIT_BUTTON_HEIGHT: f32 = git::BUTTON_HEIGHT;
/// The Git card's row gap (Pink `render_git_card`: `gap ui_rems(6)`), also
/// the next-step line's label → reason gap and the commit row's button gap.
const GIT_GAP: f32 = space::S6;

/// Pink's own Solar glyphs (`icons::ALT_ARROW_RIGHT`, `CHECK`, `MAGIC_WAND_3`).
const ICON_CHEVRON: &str = "icons/smart-recommendation/alt-arrow-right.svg";
const ICON_CHECK: &str = "icons/smart-recommendation/check.svg";
const ICON_MARK: &str = "icons/smart-recommendation/magic-wand-3.svg";

/// How sure the recommendation is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Signal {
    #[default]
    None,
    Weak,
    Good,
    Strong,
}

impl Signal {
    pub fn text(self) -> &'static str {
        match self {
            Signal::None => "No signal",
            Signal::Weak => "Weak signal",
            Signal::Good => "Good signal",
            Signal::Strong => "Strong signal",
        }
    }

    /// The short form the Alternatives menu shows beside each option.
    fn word(self) -> &'static str {
        match self {
            Signal::None => "No signal",
            Signal::Weak => "Weak",
            Signal::Good => "Good",
            Signal::Strong => "Strong",
        }
    }

    /// The badge's dot, from Pink's subagent status tones
    /// (`transcript.rs`): done `success`, running `accent`, interrupted
    /// `warning`, anything else `content_faint`.
    fn tone(self, t: &Pink) -> Hsla {
        match self {
            Signal::Strong => t.readable_tone(t.success, 1.0),
            Signal::Good => t.accent,
            Signal::Weak => t.readable_tone(t.warning, 1.0),
            Signal::None => t.content_faint,
        }
    }
}

#[derive(Clone)]
pub struct Recommendation {
    pub id: SharedString,
    /// Short name of the step, e.g. "Incremental sync": the line's label and
    /// the Accept button's object.
    pub label: SharedString,
    /// One sentence why, shown after the label; `**text**` is set in medium
    /// `text` against the `text_dim` reason.
    pub reason: SharedString,
    pub signal: Signal,
}

impl Recommendation {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        signal: Signal,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            reason: SharedString::default(),
            signal,
        }
    }

    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = reason.into();
        self
    }
}

/// Splits `**bold**` markup into the plain text and the bold ranges.
fn parse_reason(source: &str) -> (SharedString, Vec<Range<usize>>) {
    let mut text = String::new();
    let mut bold = Vec::new();
    for (ix, part) in source.split("**").enumerate() {
        let start = text.len();
        text.push_str(part);
        if ix % 2 == 1 && !part.is_empty() {
            bold.push(start..text.len());
        }
    }
    (text.into(), bold)
}

/// Text runs for the reason: regular in `dim`, the bold ranges in medium
/// `ink`, the label's weight and tone on the next-step line.
fn reason_runs(reason: &str, bold: &[Range<usize>], dim: Hsla, ink: Hsla) -> Vec<TextRun> {
    let run = |len: usize, weight: FontWeight, color: Hsla| TextRun {
        len,
        font: Font {
            weight,
            ..font(pink::SANS)
        },
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::new();
    let mut at = 0;
    for range in bold {
        if range.start > at {
            runs.push(run(range.start - at, FontWeight::NORMAL, dim));
        }
        runs.push(run(range.len(), FontWeight::MEDIUM, ink));
        at = range.end;
    }
    if reason.len() > at {
        runs.push(run(reason.len() - at, FontWeight::NORMAL, dim));
    }
    runs
}

/// "Incremental sync" → "incremental sync", for "Accept incremental sync".
/// A leading acronym ("CDN cache") keeps its case.
fn sentence_object(label: &str) -> String {
    let mut chars = label.chars();
    match (chars.next(), chars.next()) {
        (Some(_), Some(second)) if second.is_uppercase() => label.to_string(),
        (Some(first), _) => first
            .to_lowercase()
            .chain(label[first.len_utf8()..].chars())
            .collect(),
        (None, _) => String::new(),
    }
}

type Handler = Rc<dyn Fn(&Recommendation, &mut Window, &mut App)>;

struct State {
    chosen: SharedString,
    accepted: Option<SharedString>,
    open: bool,
    /// When the line last folded or unfolded: one clock for the chevron, the
    /// height and the fade (Pink's tool fold).
    toggled_at: Option<Instant>,
    menu: bool,
    /// The menu began its `MENU_OUT` fade here.
    closing: Option<Instant>,
    /// The menu's one highlight, shared by the arrows and the pointer.
    active: usize,
    /// The pointer is on the Alternatives button: a press there toggles the
    /// menu instead of counting as a click outside it.
    alt_hovered: bool,
    /// The line's height, measured each frame for the fold.
    line_height: f32,
    focus_alt: FocusHandle,
    focus_accept: FocusHandle,
    focus_undo: FocusHandle,
    focus_menu: FocusHandle,
}

impl State {
    fn toggle_fold(&mut self, open: bool) {
        if self.open != open {
            self.open = open;
            self.toggled_at = Some(Instant::now());
        }
    }

    fn open_menu(&mut self, current: usize) {
        self.menu = true;
        self.closing = None;
        self.active = current;
    }

    fn close_menu(&mut self) {
        if self.menu {
            self.menu = false;
            self.closing = Some(Instant::now());
        }
    }
}

#[derive(IntoElement)]
pub struct SmartRecommendation {
    id: ElementId,
    title: SharedString,
    options: Vec<Recommendation>,
    on_accept: Option<Handler>,
    on_undo: Option<Handler>,
    default_open: bool,
    default_choice: Option<SharedString>,
}

impl SmartRecommendation {
    /// The first option is the recommendation, the rest are offered as alternatives.
    pub fn new(id: impl Into<ElementId>, options: Vec<Recommendation>) -> Self {
        Self {
            id: id.into(),
            title: "Smart recommendation".into(),
            options,
            on_accept: None,
            on_undo: None,
            default_open: true,
            default_choice: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn on_accept(
        mut self,
        handler: impl Fn(&Recommendation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_accept = Some(Rc::new(handler));
        self
    }

    pub fn on_undo(
        mut self,
        handler: impl Fn(&Recommendation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }

    /// Starts with the line folded away when `false`.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Starts on this option instead of the recommendation.
    pub fn default_choice(mut self, id: impl Into<SharedString>) -> Self {
        self.default_choice = Some(id.into());
        self
    }
}

impl RenderOnce for SmartRecommendation {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = format!("sr-{}", self.id);
        let options = Rc::new(self.options);
        if options.is_empty() {
            return div().id(self.id);
        }

        let first = self
            .default_choice
            .filter(|id| options.iter().any(|o| &o.id == id))
            .unwrap_or_else(|| options[0].id.clone());
        let default_open = self.default_open;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| State {
                chosen: first,
                accepted: None,
                open: default_open,
                toggled_at: None,
                menu: false,
                closing: None,
                active: 0,
                alt_hovered: false,
                line_height: 0.,
                focus_alt: cx.focus_handle().tab_stop(true),
                focus_accept: cx.focus_handle().tab_stop(true),
                focus_undo: cx.focus_handle().tab_stop(true),
                focus_menu: cx.focus_handle(),
            },
        );
        let reduced = motion::reduced_motion(cx);
        let s = state.read(cx);
        let open = s.open;
        let current_ix = options.iter().position(|o| o.id == s.chosen).unwrap_or(0);
        let current = options[current_ix].clone();
        let recommended = current_ix == 0;
        let done = s.accepted.as_ref() == Some(&current.id);
        let measured = s.line_height;
        let (focus_alt, focus_accept, focus_undo, focus_menu) = (
            s.focus_alt.clone(),
            s.focus_accept.clone(),
            s.focus_undo.clone(),
            s.focus_menu.clone(),
        );
        // The menu stays mounted through its exit fade.
        let exit = s
            .closing
            .filter(|_| !s.menu && !reduced)
            .map(motion::menu_exit_progress)
            .filter(|progress| *progress < 1.);
        let show_menu = s.menu || exit.is_some();
        let fold_elapsed = s.toggled_at.map(|at| at.elapsed()).filter(|elapsed| {
            !reduced && *elapsed < FOLD_TWEEN_WINDOW.mul_f32(motion::speed_scale())
        });
        let active = s.active.min(options.len() - 1);
        let (progress, body_alpha) = motion::fold_progress(open, fold_elapsed);
        if fold_elapsed.is_some() || exit.is_some() {
            window.request_animation_frame();
        }

        // ---- Header: folds the line (Pink `render_file_header`) -------------
        let ring = t.focus_ring();
        let head = div()
            .id("head")
            .role(Role::Button)
            .aria_label(self.title.clone())
            .aria_expanded(open)
            .tab_index(0)
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .h(px(HEAD_HEIGHT))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::S8))
            .px(px(space::MD))
            .rounded_t(px(radius::inner(radius::SECTION_CARD, 1.)))
            .cursor_pointer()
            .hover_fill(format!("{key}-head"), t.ink(0.025), t.ink(0.05))
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.toggle_fold(!s.open);
                        cx.notify();
                    })
                }
            })
            // The disclosure glyph turns through the fold on the same clock.
            .child(
                div()
                    .flex_none()
                    .size(px(icon_size::SM))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(ICON_CHEVRON)
                            .size(px(icon_size::XS))
                            .text_color(t.readable_muted(0.7))
                            .with_transformation(Transformation::rotate(radians(
                                FRAC_PI_2 * progress,
                            ))),
                    ),
            )
            .child(
                svg()
                    .path(ICON_MARK)
                    .size(px(icon_size::SM))
                    .flex_none()
                    .text_color(t.text_muted),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .type_style(TextStyle::Footnote)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(self.title.clone()),
            )
            .when(!recommended, |head| {
                head.child(motion::status_swap(
                    ElementId::Name(format!("{key}-alternative").into()),
                    pink::status_pill("Alternative", t.text_muted, None, &t),
                ))
            });

        // ---- The next-step line, which folds ----------------------------------
        // Pink's tool fold (`transcript.rs`): one clock drives the chevron, the
        // height and the fade (6 px settle); the line stays mounted while the
        // close tween shrinks over it.
        let fold = (open || fold_elapsed.is_some()).then(|| {
            let inner = div()
                .flex_none()
                .relative()
                .pb(px(GIT_GAP))
                .when(body_alpha < 1., |inner| {
                    inner
                        .top(px(TOOL_BODY_RISE * (1. - body_alpha)))
                        .opacity(body_alpha)
                })
                .child(motion::status_swap(
                    ElementId::Name(format!("{key}-line-{}", current.id).into()),
                    next_step_line(&current, &t),
                ))
                .child(
                    canvas(
                        {
                            let state = state.clone();
                            move |bounds, _, cx| {
                                let height = f32::from(bounds.size.height);
                                state.update(cx, |s, cx| {
                                    if (s.line_height - height).abs() > 0.5 {
                                        s.line_height = height;
                                        cx.notify();
                                    }
                                });
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                );
            div()
                .flex_none()
                .overflow_hidden()
                .when(fold_elapsed.is_some(), |fold| {
                    fold.h(px(measured * progress))
                })
                .child(inner)
        });

        // ---- Actions -----------------------------------------------------------
        let mut actions = div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(GIT_GAP));

        if options.len() > 1 && !done {
            let alt_key = format!("{key}-alt");
            let style = git::ButtonStyle {
                fill: motion::hover_blend(&alt_key, t.surface_raised, t.surface_raised_hover),
                border: t.border,
                text: t.text,
                weight: None,
            };
            // Its hover listener also notes the pointer for the menu.
            let trigger = git_button_base("alternatives", "Alternatives", &style, &t)
                .track_focus(&focus_alt)
                .aria_expanded(s.menu)
                .on_hover({
                    let state = state.clone();
                    move |hovered, window, cx| {
                        motion::set_hover(&alt_key, *hovered, motion::reduced_motion(cx));
                        state.update(cx, |s, _| s.alt_hovered = *hovered);
                        window.refresh();
                    }
                })
                .on_click({
                    let state = state.clone();
                    let focus_menu = focus_menu.clone();
                    move |_, window, cx| {
                        let opened = state.update(cx, |s, cx| {
                            if s.menu {
                                s.close_menu();
                            } else {
                                s.open_menu(current_ix);
                            }
                            cx.notify();
                            s.menu
                        });
                        if opened {
                            window.focus(&focus_menu, cx);
                        }
                    }
                });
            actions = actions.child(div().relative().flex_none().child(trigger).when(
                show_menu,
                |wrapper| {
                    wrapper.child(alternatives_menu(
                        &key,
                        &options,
                        current_ix,
                        active,
                        exit,
                        &state,
                        &focus_menu,
                        &focus_alt,
                        &t,
                    ))
                },
            ));
        }

        if done {
            let note = div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(GIT_GAP))
                .type_style(TextStyle::Caption)
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.readable_tone(t.success, 1.))
                .child(
                    svg()
                        .path(ICON_CHECK)
                        .size(px(icon_size::XS))
                        .flex_none()
                        .text_color(t.readable_tone(t.success, 1.)),
                )
                .child("Accepted");
            let undo = git_button("undo", format!("{key}-undo"), "Undo", &t)
                .track_focus(&focus_undo)
                .on_click({
                    let state = state.clone();
                    let on_undo = self.on_undo.clone();
                    let current = current.clone();
                    let focus_accept = focus_accept.clone();
                    move |event, window, cx| {
                        state.update(cx, |s, cx| {
                            s.accepted = None;
                            cx.notify();
                        });
                        if event.is_keyboard() {
                            window.focus(&focus_accept, cx);
                        }
                        if let Some(f) = &on_undo {
                            f(&current, window, cx);
                        }
                    }
                });
            actions = actions
                .child(motion::status_swap(
                    ElementId::Name(format!("{key}-done-{}", current.id).into()),
                    note,
                ))
                .child(undo);
        } else {
            let label = format!("Accept {}", sentence_object(&current.label));
            let accept = git_step_button("accept", format!("{key}-accept"), label, recommended, &t)
                .track_focus(&focus_accept)
                .on_click({
                    let state = state.clone();
                    let on_accept = self.on_accept.clone();
                    let current = current.clone();
                    let focus_undo = focus_undo.clone();
                    move |event: &ClickEvent, window, cx| {
                        state.update(cx, |s, cx| {
                            s.accepted = Some(current.id.clone());
                            s.close_menu();
                            cx.notify();
                        });
                        if event.is_keyboard() {
                            window.focus(&focus_undo, cx);
                        }
                        if let Some(f) = &on_accept {
                            f(&current, window, cx);
                        }
                    }
                });
            actions = actions.child(accept);
        }

        let foot = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::S8))
            .min_h(px(GIT_BUTTON_HEIGHT))
            .child(signal_badge(current.signal, current.signal.text(), &t))
            .child(div().flex_1())
            .child(actions);

        // ---- Tab moves on ---------------------------------------------------------
        let on_key = |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if event.keystroke.key == "tab" {
                if event.keystroke.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                cx.stop_propagation();
            }
        };

        div()
            .id(self.id.clone())
            .w_full()
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_key_down(on_key)
            .child(
                pink::card(&t)
                    .flex()
                    .flex_col()
                    .child(head)
                    // Pink `render_git_card`: `px SPACE_MD / py SPACE_SM`.
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .px(px(space::MD))
                            .py(px(space::SM))
                            .children(fold)
                            .child(foot),
                    ),
            )
    }
}

/// The next-step line (Pink `render_next_git_action`): the label in medium
/// `text`, the reason in `text_dim`, 11 px, gap 6, the reason wrapping
/// beside the label. Pink aligns the two on their baseline; GPUI's text
/// reports none, and with one type style top alignment lands on the same
/// line.
fn next_step_line(option: &Recommendation, t: &Pink) -> Stateful<Div> {
    let (reason, bold) = parse_reason(&option.reason);
    let runs = reason_runs(&reason, &bold, t.text_dim, t.text);
    div()
        .id("next-step")
        .role(Role::Group)
        .aria_label(SharedString::from(format!("{}. {reason}", option.label)))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(GIT_GAP))
        .min_w_0()
        .type_style(TextStyle::Caption)
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(option.label.clone()),
        )
        .when(!reason.is_empty(), |line| {
            line.child(
                div()
                    .min_w_0()
                    .text_color(t.text_dim)
                    .child(StyledText::new(reason).with_runs(runs)),
            )
        })
}

/// The confidence signal as Pink's subagent status badge (`transcript.rs`):
/// a 6 px dot and an 11 px medium word in `content_muted`, gap 6.
fn signal_badge(signal: Signal, label: &'static str, t: &Pink) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S6))
        .type_style(TextStyle::Caption)
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.content_muted)
        .child(
            div()
                .size(px(space::S6))
                .flex_none()
                .rounded_full()
                .bg(signal.tone(t)),
        )
        .child(label)
}

/// The Git card's button body (Pink `Changes::git_button_base`), drawn by
/// the `pink-git-card` package: 26 tall, `px 10`, `CONTROL_RADIUS`, a 1 px
/// edge, 12 px text; `style` carries the recipe's plate already blended over
/// its hover fade. This adds the name, the tab stop and the inset focus ring;
/// the caller adds the hover listener.
fn git_button_base(
    id: &'static str,
    label: impl Into<SharedString>,
    style: &git::ButtonStyle,
    t: &Pink,
) -> Stateful<Div> {
    let label = label.into();
    let ring = t.focus_ring();
    git::button(id, label.clone(), style)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .flex_none()
        .whitespace_nowrap()
        .focus_visible(move |style| pink::focus_ring(style, ring))
}

/// A plain Git card button (Pink `git_button`): `surface_raised`, easing to
/// `surface_raised_hover` under the pointer.
fn git_button(
    id: &'static str,
    hover_key: String,
    label: impl Into<SharedString>,
    t: &Pink,
) -> Stateful<Div> {
    let style = git::ButtonStyle {
        fill: motion::hover_blend(&hover_key, t.surface_raised, t.surface_raised_hover),
        border: t.border,
        text: t.text,
        weight: None,
    };
    git_button_base(id, label, &style, t).on_hover(motion::hover_listener(hover_key))
}

/// The button that performs the shown step (Pink `git_step_button`): the
/// Git card's accent plate (`git_primary_button`: `accent_strong` → 88%,
/// medium `on_accent`, no edge) when it carries the recommendation, the plain
/// card button otherwise.
fn git_step_button(
    id: &'static str,
    hover_key: String,
    label: impl Into<SharedString>,
    accents: bool,
    t: &Pink,
) -> Stateful<Div> {
    if accents {
        let fill = t.accent_strong;
        let style = git::ButtonStyle {
            fill: motion::hover_blend(&hover_key, fill, fill.opacity(0.88)),
            border: gpui::transparent_black(),
            text: t.on_accent,
            weight: Some(FontWeight::MEDIUM),
        };
        git_button_base(id, label, &style, t).on_hover(motion::hover_listener(hover_key))
    } else {
        git_button(id, hover_key, label, t)
    }
}

/// The "Recommended" marker beside the first option: Pink's "Default" ghost
/// badge (`pickers.rs default_badge`), bare 11 px semibold `text_muted`.
fn recommended_marker(t: &Pink) -> Div {
    div()
        .flex_none()
        .text_size(px(pink::text::CAPTION))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text_muted)
        .child("Recommended")
}

/// The Alternatives menu, opening above its button's trailing edge (Pink
/// `anchored_menu_above_end`): a `popover_card` of `menu_row_nav` rows with
/// one highlight the arrows and the pointer share and a check on the shown
/// option. It rises 6 px in over `MENU_IN` and fades out over `MENU_OUT`
/// under an occluder, so the dying rows take no clicks.
#[allow(clippy::too_many_arguments)]
fn alternatives_menu(
    key: &str,
    options: &Rc<Vec<Recommendation>>,
    current_ix: usize,
    active: usize,
    exit: Option<f32>,
    state: &Entity<State>,
    focus_menu: &FocusHandle,
    focus_alt: &FocusHandle,
    t: &Pink,
) -> impl IntoElement {
    let rows = options.iter().enumerate().map(|(ix, option)| {
        pink::menu_row_nav(ix == current_ix, ix == active, t)
            .id(ElementId::Name(
                format!("{key}-option-{}", option.id).into(),
            ))
            .role(Role::MenuItem)
            .on_mouse_move({
                let state = state.clone();
                move |_: &MouseMoveEvent, _, cx| {
                    state.update(cx, |s, cx| {
                        if s.active != ix {
                            s.active = ix;
                            cx.notify();
                        }
                    })
                }
            })
            .on_click({
                let state = state.clone();
                let options = options.clone();
                let focus_alt = focus_alt.clone();
                move |_, window, cx| {
                    pick(&state, &options, ix, cx);
                    window.focus(&focus_alt, cx);
                }
            })
            .child(option.label.clone())
            .when(ix == 0, |row| row.child(recommended_marker(t)))
            .child(div().flex_1())
            .child(signal_badge(option.signal, option.signal.word(), t))
    });
    let count = options.len();
    let card = pink::popover_card(t)
        .flex()
        .flex_col()
        .min_w(px(layout::MENU_W_M))
        .track_focus(focus_menu)
        .on_key_down({
            let state = state.clone();
            let options = options.clone();
            let focus_alt = focus_alt.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let at = state.read(cx).active;
                // Pink `popover::menu_step`: the arrows wrap.
                let step = |delta: isize| (at as isize + delta).rem_euclid(count as isize) as usize;
                let next = match event.keystroke.key.as_str() {
                    "down" => step(1),
                    "up" => step(-1),
                    "home" => 0,
                    "end" => count - 1,
                    "enter" | "space" => {
                        pick(&state, &options, at, cx);
                        window.focus(&focus_alt, cx);
                        cx.stop_propagation();
                        return;
                    }
                    // Tab leaves the menu the way Escape does, back on its button.
                    "escape" | "tab" => {
                        state.update(cx, |s, cx| {
                            s.close_menu();
                            cx.notify();
                        });
                        window.focus(&focus_alt, cx);
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                state.update(cx, |s, cx| {
                    s.active = next;
                    cx.notify();
                });
                cx.stop_propagation();
            }
        })
        .on_mouse_down_out({
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if s.menu && !s.alt_hovered {
                        s.close_menu();
                        cx.notify();
                    }
                })
            }
        })
        .children(rows);
    let layer = div().occlude().pb(px(layout::POPOVER_GAP)).child(card);
    let layer = match exit {
        Some(progress) => motion::menu_out(
            ElementId::Name(format!("{key}-menu-out").into()),
            progress,
            layer.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(
            ElementId::Name(format!("{key}-menu").into()),
            MENU_RISE_UP,
            layer,
        )
        .into_any_element(),
    };
    div().absolute().top_0().right_0().size_0().child(
        deferred(
            anchored()
                .anchor(Anchor::BottomRight)
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(layer),
        )
        .with_priority(1),
    )
}

/// Shows option `ix`, closes the menu and unfolds its line.
fn pick(state: &Entity<State>, options: &[Recommendation], ix: usize, cx: &mut App) {
    let Some(option) = options.get(ix) else {
        return;
    };
    state.update(cx, |s, cx| {
        s.chosen = option.id.clone();
        s.active = ix;
        s.close_menu();
        s.toggle_fold(true);
        cx.notify();
    });
}

/// The demo's data: a nightly data sync.
pub fn demo_options() -> Vec<Recommendation> {
    vec![
        Recommendation::new("incremental", "Incremental sync", Signal::Strong).reason(
            "Only **4%** of rows changed since yesterday, so it finishes about 40 minutes sooner.",
        ),
        Recommendation::new("full", "Full refresh", Signal::Weak).reason(
            "Rebuilds every table. Safest after a schema change, but takes about **50 minutes**.",
        ),
        Recommendation::new("skip", "Skip tonight", Signal::None)
            .reason("Keeps yesterday's data. Fine if nobody needs fresh numbers tomorrow morning."),
    ]
}

/// A second card's data: which tests a pull request runs.
fn test_run_options() -> Vec<Recommendation> {
    vec![
        Recommendation::new("affected", "Affected tests only", Signal::Strong)
            .reason("The change touches 3 packages; their **412 tests** cover every changed line."),
        Recommendation::new("full", "Full suite", Signal::Good).reason(
            "All 6,880 tests in about **18 minutes**. Catches what the dependency graph misses.",
        ),
        Recommendation::new("skip", "Skip tests", Signal::None)
            .reason("Fastest, but the merge queue runs them anyway and may bounce the PR."),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .w(px(440.))
            .flex()
            .flex_col()
            .gap(px(space::LG))
            .child(
                SmartRecommendation::new("smart-recommendation", demo_options())
                    .title("Nightly sync"),
            )
            .child(
                SmartRecommendation::new("smart-recommendation-tests", test_run_options())
                    .title("Test run")
                    .default_choice("full")
                    .default_open(false),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_markup() {
        let (text, bold) = parse_reason("Only **4%** of rows changed since yesterday.");
        assert_eq!(&text[bold[0].clone()], "4%");
        assert!(!text.contains("**"));
        let (text, bold) = parse_reason("**Skip tonight** keeps data.");
        assert_eq!(&text[bold[0].clone()], "Skip tonight");
    }

    #[test]
    fn accept_label_object() {
        assert_eq!(sentence_object("Incremental sync"), "incremental sync");
        assert_eq!(sentence_object("CDN cache"), "CDN cache");
        assert_eq!(sentence_object(""), "");
    }
}
