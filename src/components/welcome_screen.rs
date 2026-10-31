//! Welcome Screen – Pink's first-run screen: what the main column shows
//! before there is a project to work in.
//!
//! Pink's mark sits over one question, a line on what to do, one way forward
//! and its shortcut. The mark is the app icon's idea at hero size: a rounded
//! tile of 9 × 9 rounded cells, light pink at its heart and deepening shade
//! by shade to its edges. Under the pointer a ripple runs out from the
//! centre once, over 1.6 s, lifting each ring as it passes. The whole group
//! arrives with Pink's 500 ms rise.
//!
//! ```ignore
//! WelcomeScreen::new("welcome")
//!     .size(940.0, 800.0)
//!     .shortcut(Some("⌘⇧N"))
//!     .on_add_project(|_, _| println!("pick a folder"))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyView, App, Context, ElementId, IntoElement, Role, SharedString, Task, Window, div,
    prelude::*, px, rgb,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, FillTier, Pink},
    story,
};

pub const NAME: &str = "Welcome Screen";
pub const DESCRIPTION: &str = "The first thing a new workspace shows: Pink's mark, a tile of pink cells that deepen to its edges and ripple once under the pointer, above one question, a line on what to do, the one button that starts a project and its shortcut";

/// The `pink-welcome-screen` package (`components/welcome-screen`): PINK's
/// mark (welcome_mark.rs), its question, line, action and shortcut,
/// compiled here unchanged. The story adds the canvas, the clock and the
/// kit's Primary button.
#[allow(dead_code)]
pub mod package {
    include!("../../components/welcome-screen/src/lib.rs");
}
use package::{Cell, MarkStyle, RIPPLE_SECONDS, mark_cells, mark_side, ripple_at};

/// Pink: workbench/render.rs `CANVAS_RADIUS` – the canvas the screen sits on.
const CANVAS_RADIUS: f32 = 14.0;
/// The demo ripples the mark itself this often (Pink waits for the pointer).
const AUTOPLAY_FIRST: Duration = Duration::from_millis(900);
const AUTOPLAY_EVERY: Duration = Duration::from_secs(6);

/// Pink's brand pinks (welcome_mark.rs `BRAND`): the mark's heart, edge and
/// ripple highlight, the same in both appearances like the app icon.
fn brand() -> MarkStyle {
    MarkStyle {
        heart: rgb(0xf9a8d4).into(),
        edge: rgb(0xbe185d).into(),
        highlight: rgb(0xfdf2f8).into(),
    }
}

/// The mark's state (Pink `WelcomeMark`): when the ripple started and the
/// last cells drawn.
struct MarkState {
    started: Option<Instant>,
    cached: Option<f32>,
    cells: Vec<Option<Cell>>,
    _autoplay: Option<Task<()>>,
}

impl MarkState {
    fn new(autoplay: bool, cx: &mut Context<Self>) -> Self {
        let autoplay = autoplay.then(|| {
            cx.spawn(async move |this, cx| {
                let mut wait = AUTOPLAY_FIRST;
                loop {
                    cx.background_executor().timer(wait).await;
                    wait = AUTOPLAY_EVERY;
                    let rippled = this.update(cx, |m, cx| {
                        if m.started.is_none() && !motion::reduced_motion(cx) {
                            m.started = Some(Instant::now());
                            cx.notify();
                        }
                    });
                    if rippled.is_err() {
                        break;
                    }
                }
            })
        });
        Self {
            started: None,
            cached: None,
            cells: mark_cells(None),
            _autoplay: autoplay,
        }
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

type AddHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct WelcomeScreen {
    id: ElementId,
    heading: SharedString,
    subheading: SharedString,
    action: SharedString,
    shortcut: Option<SharedString>,
    width: f32,
    height: f32,
    side: Option<f32>,
    autoplay: bool,
    on_add_project: Option<AddHandler>,
}

impl WelcomeScreen {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            heading: "What will you build?".into(),
            subheading: "Add a folder to start a thread with your coding agents.".into(),
            action: "Add project".into(),
            shortcut: Some("⌘⇧N".into()),
            width: 940.0,
            height: 800.0,
            side: None,
            autoplay: false,
            on_add_project: None,
        }
    }

    /// The question under the mark.
    pub fn heading(mut self, heading: impl Into<SharedString>) -> Self {
        self.heading = heading.into();
        self
    }

    /// The line under the question.
    pub fn subheading(mut self, subheading: impl Into<SharedString>) -> Self {
        self.subheading = subheading.into();
        self
    }

    /// The button's label.
    pub fn action_label(mut self, label: impl Into<SharedString>) -> Self {
        self.action = label.into();
        self
    }

    /// The button's shortcut, shown under it (`None` hides the hint).
    pub fn shortcut(mut self, keys: Option<impl Into<SharedString>>) -> Self {
        self.shortcut = keys.map(Into::into);
        self
    }

    /// The canvas's size; the mark scales with it like Pink's.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Draws the mark at this side instead of scaling it.
    pub fn mark_side(mut self, side: f32) -> Self {
        self.side = Some(side);
        self
    }

    /// Ripples the mark every few seconds on its own, not only under the
    /// pointer (for demos).
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    pub fn on_add_project(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_add_project = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for WelcomeScreen {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let autoplay = self.autoplay;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "mark".into()),
            cx,
            move |_, cx| MarkState::new(autoplay, cx),
        );
        let side = self
            .side
            .unwrap_or_else(|| mark_side(self.width, self.height));

        // Pink `WelcomeMark::render`: the ripple runs on animation frames
        // and stops asking for them when it completes (or never starts
        // under reduced motion).
        let reduced = motion::reduced_motion(cx);
        let cells = state.update(cx, |m, _| {
            let duration = RIPPLE_SECONDS * motion::speed_scale().max(0.01);
            let progress = m
                .started
                .map(|start| start.elapsed().as_secs_f32() / duration);
            let ripple = ripple_at(progress).filter(|_| !reduced);
            match ripple {
                Some(_) => window.request_animation_frame(),
                None => m.started = None,
            }
            if m.cached != ripple {
                m.cells = mark_cells(ripple);
                m.cached = ripple;
            }
            m.cells.clone()
        });

        let hover_state = state.clone();
        let mark = package::mark(cells, side, &brand())
            .id(ElementId::Name(format!("{key}-mark").into()))
            .role(Role::Image)
            .aria_label("Pink")
            .on_hover(move |hovered: &bool, _, cx| {
                let hovered = *hovered;
                hover_state.update(cx, |m, cx| {
                    if hovered && m.started.is_none() && !motion::reduced_motion(cx) {
                        m.started = Some(Instant::now());
                        cx.notify();
                    }
                });
            });

        // Pink: shell.rs – one clear affordance: the kit's Primary at the
        // dialog size, widened and set at 14.
        let on_add = self.on_add_project.clone();
        let add = pink::button(
            format!("{key}-add"),
            ButtonKind::Primary,
            ControlSize::Lg,
            &t,
        )
        .aria_label(self.action.clone())
        .child(self.action.clone())
        .on_click(move |_, window, cx| {
            if let Some(f) = on_add.as_ref() {
                f(window, cx);
            }
        });
        // The package sets it under the line at the dialog size.
        let add = package::action(add);

        let hint = self.shortcut.clone().map(|keys| {
            package::hint(
                &package::HintStyle {
                    text: t.content_faint,
                    keycap: t.fill(FillTier::Secondary),
                    keycap_text: t.text_muted,
                    mono: pink::MONO.into(),
                },
                "or press",
                keys,
            )
        });
        let group = package::welcome()
            .child(mark)
            .child(package::heading(t.text, self.heading.clone()))
            .child(package::subheading(
                t.content_muted,
                self.subheading.clone(),
            ))
            .child(add)
            .children(hint);

        // The main column's canvas (Pink: workbench/render.rs
        // `contained_plane`): radius 14, a hairline, the canvas fill.
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .rounded(px(CANVAS_RADIUS))
            .border_1()
            .border_color(t.hairline(0.08))
            .bg(t.canvas())
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(motion::fade_in_rise(
                ElementId::Name(format!("{key}-canvas").into()),
                group,
            ))
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        WelcomeScreen::new("welcome-screen")
            .size(940.0, 800.0)
            .autoplay(true)
    })
}

#[cfg(test)]
mod tests {
    use super::package::{GRID, MARK_SIDE, cell_color, mix};
    use super::*;

    fn at(cells: &[Option<Cell>], row: usize, col: usize) -> Option<Cell> {
        cells[row * GRID + col]
    }

    #[test]
    fn the_tile_is_a_rounded_square_of_cells() {
        let cells = mark_cells(None);
        assert_eq!(cells.len(), GRID * GRID);
        let last = GRID - 1;
        // Only the four corner cells fall out, which rounds the tile.
        for (row, col) in [(0, 0), (0, last), (last, 0), (last, last)] {
            assert_eq!(at(&cells, row, col), None);
        }
        assert_eq!(cells.iter().filter(|cell| cell.is_none()).count(), 4);
        // At rest nothing is lifted.
        assert!(cells.iter().flatten().all(|cell| cell.lift == 0.0));
    }

    #[test]
    fn shades_deepen_from_the_heart_to_the_edge() {
        let cells = mark_cells(None);
        let mid = GRID / 2;
        let heart = at(&cells, mid, mid).unwrap().shade;
        assert_eq!(heart, 0.0);
        // Each ring outwards along the middle row is a deeper shade, ending
        // on the edge.
        let row: Vec<f32> = (mid..GRID)
            .map(|col| at(&cells, mid, col).unwrap().shade)
            .collect();
        assert!(row.windows(2).all(|pair| pair[0] < pair[1]), "{row:?}");
        assert!(row.last().copied().unwrap() > 0.8);
        // The tile is symmetric.
        assert_eq!(at(&cells, mid, 0), at(&cells, mid, GRID - 1));
        assert_eq!(at(&cells, 0, mid), at(&cells, GRID - 1, mid));
    }

    #[test]
    fn a_ripple_runs_outwards_and_settles() {
        let mid = GRID / 2;
        let lift = |ripple: f32, col: usize| at(&mark_cells(Some(ripple)), mid, col).unwrap().lift;
        // Early on the heart is lifted more than the edge; later the edge
        // more than the heart.
        assert!(lift(0.05, mid) > lift(0.05, GRID - 1));
        assert!(lift(0.7, GRID - 1) > lift(0.7, mid));
        // The ripple fades as it ends, and the clock parks.
        assert!(lift(0.98, GRID - 1) < 0.05);
        assert_eq!(ripple_at(None), None);
        assert_eq!(ripple_at(Some(0.0)), Some(0.0));
        assert_eq!(ripple_at(Some(1.0)), None);
        assert!(ripple_at(Some(0.5)).unwrap() > 0.5);
    }

    #[test]
    fn cells_mix_between_the_brand_shades() {
        let style = brand();
        let heart = cell_color(
            Cell {
                shade: 0.0,
                lift: 0.0,
            },
            &style,
        );
        let edge = cell_color(
            Cell {
                shade: 1.0,
                lift: 0.0,
            },
            &style,
        );
        let close = |a: gpui::Hsla, b: gpui::Hsla| {
            let (a, b) = (gpui::Rgba::from(a), gpui::Rgba::from(b));
            (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
        };
        assert!(close(heart, style.heart));
        assert!(close(edge, style.edge));
        // A lifted cell moves towards the highlight.
        let lifted = cell_color(
            Cell {
                shade: 1.0,
                lift: 1.0,
            },
            &style,
        );
        assert!(gpui::Rgba::from(lifted).g > gpui::Rgba::from(edge).g);
        assert!(close(
            mix(style.heart, style.edge, 0.5),
            mix(style.edge, style.heart, 0.5)
        ));
    }

    #[test]
    fn the_mark_scales_like_pinks() {
        assert_eq!(mark_side(1200.0, 1000.0), MARK_SIDE);
        assert_eq!(mark_side(940.0, 720.0), MARK_SIDE * 0.8);
        assert_eq!(mark_side(200.0, 300.0), MARK_SIDE * 0.6);
    }
}
