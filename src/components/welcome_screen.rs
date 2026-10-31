//! Welcome Screen – Pink's first-run screen: what the main column shows
//! before there is a project to work in.
//!
//! A field of Pink's pink cells fills the room over one question, a line on
//! what to do, one way forward and its shortcut. The field is the app icon
//! grown outwards: rounded cells on the icon's grid, lightest and largest at
//! the heart and deepening, shrinking and fading as they spread, dissolving
//! well before the edges (faster below the heart, so the words stay clear).
//! The cells pop in from the centre when the screen appears, then pulses
//! keep rolling out from the heart, one every 1.6 s. The whole group arrives
//! with Pink's 500 ms rise.
//!
//! ```ignore
//! WelcomeScreen::new("welcome")
//!     .size(940.0, 800.0)
//!     .shortcut(Some("⇧⌘N"))
//!     .on_add_project(|_, _| println!("pick a folder"))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::Instant;

use gpui::{
    AnyView, App, BoxShadow, ElementId, IntoElement, Role, SharedString, Window, div, point,
    prelude::*, px, rgb,
};

use crate::{
    motion,
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Welcome Screen";
pub const DESCRIPTION: &str = "The first thing a new workspace shows: a field of Pink's pink cells that pops in from the centre and keeps pulsing outwards, above a pink-tinted question and line, a pink cell button whose glow breathes with the pulse, and its shortcut in key cells";

/// The `pink-welcome-screen` package (`components/welcome-screen`): PINK's
/// pulsing cell field (welcome_mark.rs), its question, line, action and
/// shortcut, compiled here unchanged. The story adds the canvas, the clocks
/// and the kit's Primary button.
#[allow(dead_code)]
pub mod package {
    include!("../../components/welcome-screen/src/lib.rs");
}
use package::{BUILD_SECONDS, FieldStyle, build_at, field_cells, field_layout, pulse_at};

/// Pink: workbench/render.rs `CANVAS_RADIUS` – the canvas the screen sits on.
const CANVAS_RADIUS: f32 = 14.0;

/// Pink's brand pinks (welcome_mark.rs `brand`), the app icon's: the cells'
/// seven shades from the heart outwards and the pulse's highlight.
fn brand() -> FieldStyle {
    let shades = [
        0xfbcfe8, 0xf9a8d4, 0xf472b6, 0xec4899, 0xdb2777, 0xbe185d, 0x9d174d,
    ];
    FieldStyle {
        shades: shades.iter().map(|hex| rgb(*hex).into()).collect(),
        highlight: rgb(0xfdf2f8).into(),
    }
}

/// The field's clocks (Pink `WelcomeMark`): when its build and its pulse
/// started.
struct FieldClock {
    built: Option<Instant>,
    epoch: Instant,
}

impl FieldClock {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            built: Some(now),
            epoch: now,
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
    on_add_project: Option<AddHandler>,
}

impl WelcomeScreen {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            heading: "What will you build?".into(),
            subheading: "Add a folder to start a thread with your coding agents.".into(),
            action: "Add project".into(),
            shortcut: Some("⇧⌘N".into()),
            width: 940.0,
            height: 800.0,
            on_add_project: None,
        }
    }

    /// The question under the field's heart.
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

    /// The canvas's size; the field scales with it like Pink's.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
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
        let clock = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "field".into()),
            cx,
            |_, _| FieldClock::new(),
        );
        let layout = field_layout(self.width, self.height);

        // Pink `WelcomeMark::render`: the build, then a pulse that keeps
        // running on animation frames; reduced motion shows the field at
        // rest and asks for none.
        let reduced = motion::reduced_motion(cx);
        let (build, pulse) = clock.update(cx, |c, _| {
            let speed = motion::speed_scale().max(0.01);
            let build = build_at(
                c.built
                    .map(|start| start.elapsed().as_secs_f32() / (BUILD_SECONDS * speed)),
            )
            .filter(|_| !reduced);
            if build.is_none() {
                c.built = None;
            }
            let pulse = (!reduced).then(|| pulse_at(c.epoch.elapsed().as_secs_f32() / speed));
            (build, pulse)
        });
        if build.is_some() || pulse.is_some() {
            window.request_animation_frame();
        }
        let hero = package::hero(&layout, field_cells(&layout, build, pulse), &brand())
            .id(ElementId::Name(format!("{key}-field").into()))
            .role(Role::Image)
            .aria_label("Pink");

        // Pink: welcome_mark.rs – the one action as a pink cell whose glow
        // breathes with each pulse leaving the heart.
        let on_add = self.on_add_project.clone();
        let glow = package::glow_strength(pulse);
        let add = package::action(
            div()
                .id(ElementId::Name(format!("{key}-add").into()))
                .role(Role::Button)
                .aria_label(self.action.clone())
                .cursor_pointer()
                .child(self.action.clone())
                .on_click(move |_, window, cx| {
                    if let Some(f) = on_add.as_ref() {
                        f(window, cx);
                    }
                }),
            &package::ActionStyle {
                top: rgb(0xf472b6).into(),
                bottom: rgb(0xdb2777).into(),
                edge: gpui::Hsla::from(rgb(0xbe185d)).opacity(0.5),
                text: gpui::white(),
            },
        )
        .shadow(vec![BoxShadow {
            color: gpui::Hsla::from(rgb(0xec4899)).opacity(glow),
            offset: point(px(0.0), px(package::GLOW_OFFSET)),
            blur_radius: px(package::GLOW_BLUR),
            spread_radius: px(package::GLOW_SPREAD),
            inset: false,
        }]);

        // The field's pink, leaned into the words.
        let pink_tint: gpui::Hsla = rgb(if t.dark { 0xf9a8d4 } else { 0xbe185d }).into();
        let hint = self.shortcut.clone().map(|keys| {
            package::hint(
                &package::HintStyle {
                    text: t.content_faint,
                    cell: gpui::Hsla::from(rgb(0xec4899)).opacity(if t.dark { 0.14 } else { 0.09 }),
                    cell_edge: pink_tint.opacity(if t.dark { 0.28 } else { 0.22 }),
                    cell_text: pink_tint,
                    mono: pink::MONO.into(),
                },
                "or press",
                package::split_keys(&keys),
            )
        });
        let group = package::welcome()
            .child(hero)
            .child(package::heading(
                package::mix(t.text, pink_tint, package::HEADING_TINT),
                self.heading.clone(),
            ))
            .child(package::subheading(
                package::mix(t.content_muted, pink_tint, package::SUBHEADING_TINT),
                self.subheading.clone(),
            ))
            .child(add)
            .children(hint);

        // The main column's canvas (Pink: workbench/render.rs
        // `contained_plane`): radius 14, a hairline, the canvas fill. It
        // clips the field.
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
        WelcomeScreen::new("welcome-screen").size(940.0, 800.0)
    })
}

#[cfg(test)]
mod tests {
    use super::package::{
        CELL_SHARE, FIELD_DOWNWARD, FIELD_PITCH, FIELD_REACH_MAX, FieldCell, HERO_SIDE,
        appear_scale, cell_color, field_alpha, pulse_lift, shade_index,
    };
    use super::*;

    /// A 1200 x 1000 room: the field reaches 600 across and 410 up.
    const ROOM: (f32, f32) = (1200.0, 1000.0);
    const ACROSS: f32 = 600.0;
    const UP: f32 = 410.0;

    fn rest() -> Vec<FieldCell> {
        field_cells(&field_layout(ROOM.0, ROOM.1), None, None)
    }

    fn nearest(cells: &[FieldCell], x: f32, y: f32) -> FieldCell {
        *cells
            .iter()
            .min_by(|a, b| {
                (a.x - x)
                    .hypot(a.y - y)
                    .total_cmp(&(b.x - x).hypot(b.y - y))
            })
            .unwrap()
    }

    #[test]
    fn the_field_spreads_from_a_four_cell_heart() {
        let cells = rest();
        // Four cells share the heart, as in the icon, at full size and the
        // lightest shade.
        let heart: Vec<&FieldCell> = cells
            .iter()
            .filter(|cell| cell.x.abs() < FIELD_PITCH && cell.y.abs() < FIELD_PITCH)
            .collect();
        assert_eq!(heart.len(), 4);
        for cell in &heart {
            assert!((cell.x.abs() - FIELD_PITCH * 0.5).abs() < 1e-3);
            assert_eq!(shade_index(cell.distance, 7), 0);
            assert!(cell.extent > FIELD_PITCH * CELL_SHARE * 0.98);
        }
        // It reaches for the room's side and top edges, much less far below,
        // and every cell sits inside the reach.
        assert!(cells.iter().any(|cell| cell.x > ACROSS * 0.85));
        assert!(cells.iter().all(|cell| cell.x.abs() < ACROSS));
        assert!(cells.iter().any(|cell| cell.y < -UP * 0.8));
        assert!(cells.iter().all(|cell| cell.y > -UP));
        let lowest = cells.iter().map(|cell| cell.y).fold(f32::MIN, f32::max);
        assert!(lowest < FIELD_PITCH + (UP - FIELD_PITCH) / FIELD_DOWNWARD + 1e-3);
        assert!(cells.iter().all(|cell| cell.distance < 1.0));
        assert!(cells.len() > 200);
    }

    #[test]
    fn cells_deepen_shrink_and_fade_outwards() {
        let cells = rest();
        let heart = nearest(&cells, FIELD_PITCH * 0.5, -FIELD_PITCH * 0.5);
        let middle = nearest(&cells, ACROSS * 0.45, 0.0);
        let outer = nearest(&cells, ACROSS * 0.9, 0.0);
        assert!(shade_index(heart.distance, 7) < shade_index(middle.distance, 7));
        assert!(heart.extent > middle.extent && middle.extent > outer.extent);
        assert_eq!(field_alpha(heart.distance), 1.0);
        assert!(field_alpha(middle.distance) < 1.0);
        assert!(field_alpha(outer.distance) < 0.1);
        assert_eq!(field_alpha(1.0), 0.0);
        // Below the heart it fades sooner than above it.
        let above = nearest(&cells, FIELD_PITCH * 0.5, -110.0);
        let below = nearest(&cells, FIELD_PITCH * 0.5, 110.0);
        assert!(below.distance > above.distance);
    }

    #[test]
    fn the_build_fills_from_the_heart_outwards() {
        let layout = field_layout(ROOM.0, ROOM.1);
        let early = field_cells(&layout, Some(0.3), None);
        let heart = nearest(&early, FIELD_PITCH * 0.5, FIELD_PITCH * 0.5);
        let outer = nearest(&early, ACROSS * 0.7, 0.0);
        assert!(heart.appear > outer.appear);
        assert_eq!(field_cells(&layout, Some(0.0), None)[0].appear, 0.0);
        assert!((1..100).any(|step| appear_scale(step as f32 / 100.0) > 1.0));
        assert!((appear_scale(1.0) - 1.0).abs() < 1e-5);
        assert_eq!(build_at(None), None);
        assert_eq!(build_at(Some(1.0)), None);
        assert_eq!(build_at(Some(0.4)), Some(0.4));
    }

    #[test]
    fn pulses_roll_out_from_the_heart_and_dim() {
        // A pulse just leaving lifts the heart; later it lifts a ring
        // farther out, and the heart only when the next pulse leaves.
        assert!(pulse_lift(0.0, 0.02) > 0.5);
        assert!(pulse_lift(0.5, 0.48) > pulse_lift(0.05, 0.48));
        assert!(pulse_lift(0.06, 0.54) > 0.5);
        // It dims as it travels.
        assert!(pulse_lift(0.9, 0.82) < pulse_lift(0.2, 0.22));
        // The pulse cycles.
        let three = super::package::PULSE_SECONDS * 3.0;
        assert!((pulse_at(three) - pulse_at(0.0)).abs() < 1e-4);
        assert!(pulse_at(1.0) > 0.0 && pulse_at(1.0) < 1.0);
    }

    #[test]
    fn cells_take_the_brand_shades_and_brighten_with_a_pulse() {
        let style = brand();
        let cells = rest();
        let heart = nearest(&cells, FIELD_PITCH * 0.5, FIELD_PITCH * 0.5);
        let close = |a: gpui::Hsla, b: gpui::Hsla| {
            let (a, b) = (gpui::Rgba::from(a), gpui::Rgba::from(b));
            (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01
        };
        assert!(close(cell_color(&heart, &style), style.shades[0]));
        let edge = nearest(&cells, ACROSS * 0.5, 0.0);
        let mut lifted = edge;
        lifted.lift = 1.0;
        assert!(cell_color(&lifted, &style).a > cell_color(&edge, &style).a);
        let mut hidden = heart;
        hidden.appear = 0.0;
        assert_eq!(cell_color(&hidden, &style).a, 0.0);
    }

    #[test]
    fn the_action_glows_with_the_pulse_and_keys_split_into_cells() {
        use super::package::{GLOW_PEAK, GLOW_REST, glow_strength, split_keys};
        assert_eq!(glow_strength(None), GLOW_REST);
        // A pulse leaving the heart brightens the glow; between pulses it rests.
        assert!(glow_strength(Some(0.06)) > GLOW_REST + 0.5 * (GLOW_PEAK - GLOW_REST));
        assert!(glow_strength(Some(0.3)) < GLOW_REST + 0.1);
        assert!(glow_strength(Some(0.06)) <= GLOW_PEAK);
        let keys = |combo| {
            split_keys(combo)
                .into_iter()
                .map(|key| key.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(keys("⇧⌘N"), ["⇧", "⌘", "N"]);
        assert_eq!(keys("⌘Space"), ["⌘", "Space"]);
        assert_eq!(keys("F5"), ["F5"]);
        assert!(keys("").is_empty());
    }

    #[test]
    fn the_field_fits_the_room() {
        let full = field_layout(ROOM.0, ROOM.1);
        assert_eq!(full.side, HERO_SIDE);
        assert_eq!(full.pitch, FIELD_PITCH);
        assert_eq!((full.reach_x, full.reach_y), (ACROSS, UP));
        // A huge room caps the reach; a small one shrinks the cells and
        // keeps a minimum field.
        let huge = field_layout(4000.0, 3000.0);
        assert_eq!(
            (huge.reach_x, huge.reach_y),
            (FIELD_REACH_MAX, FIELD_REACH_MAX)
        );
        let small = field_layout(200.0, 300.0);
        assert!((small.side - HERO_SIDE * 0.6).abs() < 1e-3);
        assert!((small.reach_y - 240.0 * 0.6).abs() < 1e-3);
    }
}
