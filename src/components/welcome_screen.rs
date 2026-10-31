//! Welcome Screen – Pink's first-run screen: what the main column shows
//! before there is a project to work in.
//!
//! Pink's mark sits over one question, a line on what to do, one way forward
//! and its shortcut. The mark is the app icon drawn live: a deep pink tile
//! holding 6 × 6 rounded cells, lightest at its heart and deepening shade by
//! shade to its edges. It builds in when it appears (the tile settles, the
//! cells pop in from the centre outwards over 0.7 s), and under the pointer
//! a ripple runs out from the centre once, over 1.4 s, lifting each ring as
//! it passes. The whole group arrives with Pink's 500 ms rise.
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
pub const DESCRIPTION: &str = "The first thing a new workspace shows: Pink's app icon drawn live, a tile of pink cells that pop in from the centre and ripple under the pointer, above one question, a line on what to do, the one button that starts a project and its shortcut";

/// The `pink-welcome-screen` package (`components/welcome-screen`): PINK's
/// mark (welcome_mark.rs), its question, line, action and shortcut,
/// compiled here unchanged. The story adds the canvas, the clock and the
/// kit's Primary button.
#[allow(dead_code)]
pub mod package {
    include!("../../components/welcome-screen/src/lib.rs");
}
use package::{
    BUILD_SECONDS, MarkFrame, MarkStyle, RIPPLE_SECONDS, build_at, mark_frame, mark_side, ripple_at,
};

/// Pink: workbench/render.rs `CANVAS_RADIUS` – the canvas the screen sits on.
const CANVAS_RADIUS: f32 = 14.0;
/// The demo ripples the mark itself this often (Pink waits for the pointer).
const AUTOPLAY_FIRST: Duration = Duration::from_millis(900);
const AUTOPLAY_EVERY: Duration = Duration::from_secs(6);

/// Pink's brand pinks (welcome_mark.rs `brand`), the app icon's: the tile,
/// the cells' seven shades from the heart to the edge, and the ripple's
/// highlight. A dark appearance takes the icon's dark variant, whose tile
/// sits a step deeper under the same cells.
fn brand(dark: bool) -> MarkStyle {
    let shades = [
        0xfbcfe8, 0xf9a8d4, 0xf472b6, 0xec4899, 0xdb2777, 0xbe185d, 0x9d174d,
    ];
    MarkStyle {
        tile: rgb(if dark { 0x500724 } else { 0x831843 }).into(),
        shades: shades.iter().map(|hex| rgb(*hex).into()).collect(),
        highlight: rgb(0xfdf2f8).into(),
    }
}

/// The mark's state (Pink `WelcomeMark`): when its build and the latest
/// ripple started, and the last frame drawn.
struct MarkState {
    built: Option<Instant>,
    started: Option<Instant>,
    cached: (Option<f32>, Option<f32>),
    frame: MarkFrame,
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
            built: Some(Instant::now()),
            started: None,
            cached: (None, None),
            frame: mark_frame(None, None),
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

        // Pink `WelcomeMark::render`: the build and a ripple run on
        // animation frames and stop asking for them when they complete (or
        // never run under reduced motion).
        let reduced = motion::reduced_motion(cx);
        let frame = state.update(cx, |m, _| {
            let speed = motion::speed_scale().max(0.01);
            let at = |start: Option<Instant>, seconds: f32| {
                start.map(|start| start.elapsed().as_secs_f32() / (seconds * speed))
            };
            let build = build_at(at(m.built, BUILD_SECONDS)).filter(|_| !reduced);
            let ripple = ripple_at(at(m.started, RIPPLE_SECONDS)).filter(|_| !reduced);
            if build.is_none() {
                m.built = None;
            }
            if ripple.is_none() {
                m.started = None;
            }
            if build.is_some() || ripple.is_some() {
                window.request_animation_frame();
            }
            if m.cached != (build, ripple) {
                m.frame = mark_frame(build, ripple);
                m.cached = (build, ripple);
            }
            m.frame.clone()
        });

        let hover_state = state.clone();
        let mark = package::mark(frame, side, &brand(t.dark))
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
    use super::package::{
        Cell, GRID, MARK_SIDE, appear_scale, cell_color, cell_rect, clip_to_disc, mix, shade_index,
    };
    use super::*;

    fn at(frame: &MarkFrame, row: usize, col: usize) -> Cell {
        frame.cells[row * GRID + col]
    }

    #[test]
    fn the_tile_matches_the_app_icon() {
        let frame = mark_frame(None, None);
        assert_eq!(frame.cells.len(), GRID * GRID);
        assert_eq!(frame.tile, 1.0);
        // At rest every cell is in, unlifted.
        assert!(
            frame
                .cells
                .iter()
                .all(|cell| cell.appear == 1.0 && cell.lift == 0.0)
        );
        // The icon's geometry: 16 pt gaps on an 824 pt tile, six cells.
        let (x, y, extent) = cell_rect(824.0, 0, 0);
        assert!((x - 16.0).abs() < 1e-3 && (y - 16.0).abs() < 1e-3);
        assert!((extent - (824.0 - 7.0 * 16.0) / 6.0).abs() < 1e-3);
        let (x, _, extent) = cell_rect(824.0, GRID - 1, 0);
        assert!((x + extent + 16.0 - 824.0).abs() < 1e-3);
    }

    #[test]
    fn shades_deepen_from_the_heart_to_the_corners() {
        let frame = mark_frame(None, None);
        let shade = |row, col| shade_index(at(&frame, row, col).distance, 7);
        // The four heart cells share the lightest shade in use; the corners
        // take the deepest.
        let heart = shade(2, 2);
        assert_eq!([shade(2, 3), shade(3, 2), shade(3, 3)], [heart; 3]);
        assert_eq!(shade(0, 0), 6);
        assert!(heart < shade(2, 0) && shade(2, 0) < shade(0, 0));
        // The tile is symmetric.
        assert_eq!(at(&frame, 0, 2), at(&frame, 2, 0));
        assert_eq!(at(&frame, 1, 4), at(&frame, 4, 1));
    }

    #[test]
    fn the_build_fills_from_the_heart_outwards() {
        let early = mark_frame(Some(0.4), None);
        assert!(at(&early, 2, 2).appear > at(&early, 0, 0).appear);
        assert!(early.tile < 1.0);
        assert_eq!(mark_frame(Some(0.0), None).cells[0].appear, 0.0);
        // A cell overshoots a little on its way in, and lands on 1.
        assert!((1..100).any(|step| appear_scale(step as f32 / 100.0) > 1.0));
        assert!((appear_scale(1.0) - 1.0).abs() < 1e-5);
        assert_eq!(build_at(None), None);
        assert_eq!(build_at(Some(1.0)), None);
        assert_eq!(build_at(Some(0.3)), Some(0.3));
    }

    #[test]
    fn a_ripple_runs_outwards_and_settles() {
        let lift = |ripple: f32, row: usize, col: usize| {
            at(&mark_frame(None, Some(ripple)), row, col).lift
        };
        assert!(lift(0.1, 2, 2) > lift(0.1, 0, 0));
        assert!(lift(0.55, 0, 0) > lift(0.55, 2, 2));
        assert!(lift(0.98, 0, 0) < 0.05);
        assert_eq!(ripple_at(None), None);
        assert_eq!(ripple_at(Some(1.0)), None);
        assert!(ripple_at(Some(0.5)).unwrap() > 0.5);
    }

    #[test]
    fn corner_cells_are_clipped_to_the_tile() {
        // The icon's top-left corner cell against its 185.4 pt corner.
        let (x, y, extent) = cell_rect(824.0, 0, 0);
        let round = 185.4;
        let square: Vec<(f32, f32)> = vec![
            (x, y),
            (x + extent, y),
            (x + extent, y + extent),
            (x, y + extent),
        ];
        let clipped = clip_to_disc(&square, (round, round), round);
        // Every point lies inside the tile's corner, the inner corner of the
        // cell survives and the outer one is cut away.
        assert!(
            clipped
                .iter()
                .all(|p| (p.0 - round).hypot(p.1 - round) <= round + 1e-2)
        );
        assert!(clipped.contains(&(x + extent, y + extent)));
        assert!(!clipped.contains(&(x, y)));
        // A cell well inside the disc is kept whole.
        let inner = vec![
            (150.0, 150.0),
            (170.0, 150.0),
            (170.0, 170.0),
            (150.0, 170.0),
        ];
        assert_eq!(clip_to_disc(&inner, (round, round), round), inner);
    }

    #[test]
    fn cells_take_the_brand_shades() {
        let style = brand(false);
        let rest = mark_frame(None, None);
        let corner = cell_color(at(&rest, 0, 0), &style);
        let close = |a: gpui::Hsla, b: gpui::Hsla| {
            let (a, b) = (gpui::Rgba::from(a), gpui::Rgba::from(b));
            (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
        };
        assert!(close(corner, style.shades[6]));
        // A lifted cell moves towards the highlight; a cell not yet in is
        // transparent.
        let mut lifted = at(&rest, 0, 0);
        lifted.lift = 1.0;
        assert!(gpui::Rgba::from(cell_color(lifted, &style)).g > gpui::Rgba::from(corner).g);
        let mut hidden = at(&rest, 0, 0);
        hidden.appear = 0.0;
        assert_eq!(cell_color(hidden, &style).a, 0.0);
        assert!(close(mix(style.tile, style.highlight, 0.0), style.tile));
        // The dark variant's tile sits a step deeper.
        assert!(!close(brand(true).tile, style.tile));
    }

    #[test]
    fn the_mark_scales_like_pinks() {
        assert_eq!(mark_side(1200.0, 1000.0), MARK_SIDE);
        assert_eq!(mark_side(940.0, 720.0), MARK_SIDE * 0.8);
        assert_eq!(mark_side(200.0, 300.0), MARK_SIDE * 0.6);
    }
}
