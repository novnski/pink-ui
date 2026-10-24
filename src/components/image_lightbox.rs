//! Image Lightbox – a full-size view of an image over a dim scrim.
//!
//! Pink: `attachments.rs` `lightbox_with_size`, the viewer a transcript
//! thumbnail opens, over `image_viewer.rs` `ImageView`, the shared image
//! geometry and pointer gestures. The scrim (`scrim_alpha(0.7)`) fades in
//! over 200ms; the image box – 90% of the width, 85% of the height – fades in
//! with a 10px rise over 260ms expo-out. Under it, the file name in 12px
//! `ink(0.7)`, cut to one line; top right, a 32px glass capsule with the
//! close glyph. The image starts fitted (never upscaled). ⌃-scroll or a pinch
//! zooms about the pointer, up to 32×; scrolling or a drag of 4px or more
//! pans, kept inside the image's overhang. Escape or a plain click closes it;
//! a drag never does. Like Pink's, it covers the whole window, whatever
//! element renders it; render it while the image is open.
//!
//! ```ignore
//! div().child(content).when(open, |el| {
//!     el.child(
//!         ImageLightbox::new("preview", LightboxImage::new("error.png", "images/error.png"))
//!             .on_close(|_, cx| close_preview(cx)),
//!     )
//! })
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyElement, AnyView, App, BoxShadow, Context, DispatchPhase, Div, ElementId,
    FocusHandle, ImageSource, ImgResourceLoader, IntoElement, MouseButton, MouseMoveEvent,
    ObjectFit, PinchEvent, Pixels, Point, Role, ScrollDelta, ScrollWheelEvent, SharedString, Size,
    Stateful, Task, TouchPhase, Window, anchored, canvas, deferred, div, img, point, prelude::*,
    px, size, svg,
};

use crate::{
    HoverFill,
    motion::{LIGHTBOX_BACKDROP_IN, LIGHTBOX_IN},
    pink::{self, Pink, TextStyle, radius},
    story,
};

pub const NAME: &str = "Image Lightbox";
pub const DESCRIPTION: &str = "A full-size image over a dim scrim – it rises in fitted to the window, zooms about the pointer with ⌃-scroll or a pinch, pans with a drag, names its file underneath and closes with Escape, a click or its glass close capsule";

/// Pink: `icons::CLOSE`.
const ICON_CLOSE: &str = "icons/image-lightbox/close.svg";
const IMAGE_CHECKOUT: &str = "images/image-lightbox/checkout-coupon-error.png";
const IMAGE_LATENCY: &str = "images/image-lightbox/p95-latency.png";

/// Pink: `scrim_alpha(0.7)`, the lightbox's dark-mode scrim strength.
const SCRIM: f32 = 0.7;
/// Pink: the image box is 90% of the window's width and 85% of its height.
const BOX_W: f32 = 0.9;
const BOX_H: f32 = 0.85;
/// Pink: the image box rises 10px as it fades in.
const RISE: f32 = 10.0;
/// Pink: box → caption gap.
const GAP: f32 = 12.0;
/// Pink: the close capsule – 32px, 16px in from the top right corner.
const CLOSE_SIZE: f32 = 32.0;
const CLOSE_INSET: f32 = 16.0;
const CLOSE_GLYPH: f32 = 12.0;
/// Pink `image_viewer.rs`: the largest zoom, the layout bound for huge
/// images, the drag threshold and the wheel's zoom rate and line height.
const MAX_ZOOM: f32 = 32.0;
const MAX_EXTENT: f32 = 131_072.0;
const DRAG_SLOP: f32 = 4.0;
const WHEEL_ZOOM_RATE: f32 = 0.0025;
const LINE_PX: f32 = 40.0;
/// Pink `transcript.rs` `ATT_THUMB_W` / `ATT_THUMB_H`.
pub const THUMB_W: f32 = 112.0;
pub const THUMB_H: f32 = 80.0;

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The image a lightbox shows.
#[derive(Clone)]
pub struct LightboxImage {
    /// The file name under the image.
    pub name: SharedString,
    pub source: ImageSource,
    /// The logical size to fit and zoom against. Without one the decoded
    /// image's pixel size is used, and "Loading image…" shows until then.
    pub natural_size: Option<Size<f32>>,
}

impl LightboxImage {
    pub fn new(name: impl Into<SharedString>, source: impl Into<ImageSource>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            natural_size: None,
        }
    }

    /// Pink `lightbox_with_size`: a vector image keeps its source's logical
    /// dimensions.
    pub fn natural_size(mut self, width: f32, height: f32) -> Self {
        self.natural_size = Some(size(width, height));
        self
    }
}

/// The lightbox: a window-wide layer over everything else.
#[derive(IntoElement)]
pub struct ImageLightbox {
    id: ElementId,
    image: LightboxImage,
    on_close: Option<CloseHandler>,
}

impl ImageLightbox {
    pub fn new(id: impl Into<ElementId>, image: LightboxImage) -> Self {
        Self {
            id: id.into(),
            image,
            on_close: None,
        }
    }

    /// Escape, a plain click or the close capsule. Stop rendering the
    /// lightbox here; it hands focus back to whatever had it on opening.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Geometry (Pink `image_viewer.rs`)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct Geometry {
    natural: Size<f32>,
    viewport: Size<f32>,
    scale: f32,
    pan: Point<f32>,
    fitted: bool,
}

impl Default for Geometry {
    fn default() -> Self {
        Self {
            natural: size(1.0, 1.0),
            viewport: size(1.0, 1.0),
            scale: 1.0,
            pan: point(0.0, 0.0),
            fitted: true,
        }
    }
}

impl Geometry {
    /// Fit inside the viewport, never upscaled.
    fn fit_scale(&self) -> f32 {
        (self.viewport.width / self.natural.width)
            .min(self.viewport.height / self.natural.height)
            .min(1.0)
    }

    fn resize(&mut self, natural: Size<f32>, viewport: Size<f32>) {
        let sizes = [
            natural.width,
            natural.height,
            viewport.width,
            viewport.height,
        ];
        if !sizes.iter().all(|v| v.is_finite() && *v > 0.0) {
            return;
        }
        self.natural = natural;
        self.viewport = viewport;
        if self.fitted {
            self.fit();
        } else {
            self.clamp_pan();
        }
    }

    fn fit(&mut self) {
        self.scale = self.fit_scale();
        self.pan = point(0.0, 0.0);
        self.fitted = true;
    }

    fn clamp_pan(&mut self) {
        let limit = point(
            ((self.natural.width * self.scale - self.viewport.width) / 2.0).max(0.0),
            ((self.natural.height * self.scale - self.viewport.height) / 2.0).max(0.0),
        );
        self.pan.x = self.pan.x.clamp(-limit.x, limit.x);
        self.pan.y = self.pan.y.clamp(-limit.y, limit.y);
    }

    /// Zoom to `scale` keeping the image point under `anchor` in place.
    fn zoom(&mut self, scale: f32, anchor: Point<f32>) {
        if !scale.is_finite() || scale <= 0.0 || !anchor.x.is_finite() || !anchor.y.is_finite() {
            return;
        }
        // Bound layout coordinates even for enormous logical dimensions.
        let maximum = (MAX_EXTENT / self.natural.width.max(self.natural.height))
            .min(MAX_ZOOM)
            .max(self.fit_scale());
        let scale = scale.clamp(self.fit_scale().min(0.01), maximum);
        let ratio = scale / self.scale;
        let anchor = anchor - point(self.viewport.width / 2.0, self.viewport.height / 2.0);
        self.pan = point(
            anchor.x - (anchor.x - self.pan.x) * ratio,
            anchor.y - (anchor.y - self.pan.y) * ratio,
        );
        self.scale = scale;
        self.fitted = false;
        self.clamp_pan();
    }

    fn pan_by(&mut self, delta: Point<f32>) -> bool {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return false;
        }
        let before = self.pan;
        self.pan = self.pan + delta;
        self.clamp_pan();
        self.pan != before
    }

    fn image_origin(&self) -> Point<f32> {
        point(
            (self.viewport.width - self.natural.width * self.scale) / 2.0 + self.pan.x,
            (self.viewport.height - self.natural.height * self.scale) / 2.0 + self.pan.y,
        )
    }
}

fn scroll_pixels(delta: ScrollDelta) -> Point<f32> {
    match delta {
        ScrollDelta::Pixels(p) => point(f32::from(p.x), f32::from(p.y)),
        ScrollDelta::Lines(p) => point(p.x * LINE_PX, p.y * LINE_PX),
    }
}

/// Pink `image_viewer.rs` `ViewState`: geometry plus the gesture in flight.
#[derive(Default)]
struct ViewState {
    geometry: Geometry,
    bounds: gpui::Bounds<Pixels>,
    drag: Option<(Point<f32>, Point<f32>)>,
    dragged: bool,
    pinch: Option<(f32, f32)>,
}

impl ViewState {
    fn local(&self, position: Point<Pixels>) -> Point<f32> {
        point(
            f32::from(position.x - self.bounds.origin.x),
            f32::from(position.y - self.bounds.origin.y),
        )
    }

    fn pinch(&mut self, event: &PinchEvent) {
        if event.phase == TouchPhase::Ended {
            self.pinch = None;
            return;
        }
        if !event.delta.is_finite() {
            return;
        }
        if event.phase == TouchPhase::Started {
            self.pinch = Some((self.geometry.scale, 1.0));
        }
        let (start, factor) = self.pinch.get_or_insert((self.geometry.scale, 1.0));
        *factor = (*factor + event.delta).max(0.001);
        let scale = *start * *factor;
        let local = self.local(event.position);
        self.geometry.zoom(scale, local);
    }

    /// ⌃-scroll zooms about the pointer; plain scrolling pans.
    fn wheel(&mut self, event: &ScrollWheelEvent) -> bool {
        let delta = scroll_pixels(event.delta);
        if event.modifiers.control {
            let scale = self.geometry.scale * (delta.y * WHEEL_ZOOM_RATE).clamp(-2.0, 2.0).exp();
            self.geometry.zoom(scale, self.local(event.position));
            true
        } else {
            self.geometry.pan_by(delta)
        }
    }

    fn pointer_down(&mut self, position: Point<Pixels>) {
        self.dragged = false;
        self.drag = Some((self.local(position), self.geometry.pan));
    }

    fn pointer_move(&mut self, event: &MouseMoveEvent) -> bool {
        if !event.dragging() {
            self.drag = None;
            return false;
        }
        let Some((start, pan)) = self.drag else {
            return false;
        };
        let delta = self.local(event.position) - start;
        if delta.x.hypot(delta.y) >= DRAG_SLOP {
            self.dragged = true;
        }
        if !self.dragged {
            return false;
        }
        self.geometry.pan = pan;
        self.geometry.pan_by(delta);
        true
    }

    /// Pink `ImageView::begin_click`.
    fn begin_click(&mut self) {
        self.drag = None;
        self.dragged = false;
    }
}

struct LightboxState {
    view: ViewState,
    /// The image the view was fitted to; another one starts fitted again
    /// (Pink: each preview carries its own viewer).
    shown: SharedString,
    focus: FocusHandle,
    /// Whatever had focus when the lightbox opened.
    return_focus: Option<FocusHandle>,
}

/// The decoded image's pixel size (Pink: `use_render_image(..).size(0)`).
fn decoded_size(source: &ImageSource, window: &mut Window, cx: &mut App) -> Option<Size<f32>> {
    let image = match source {
        ImageSource::Resource(resource) => {
            window.use_asset::<ImgResourceLoader>(resource, cx)?.ok()?
        }
        ImageSource::Render(image) => image.clone(),
        ImageSource::Image(image) => image.clone().use_render_image(window, cx)?,
        ImageSource::Custom(load) => load(window, cx)?.ok()?,
    };
    let pixels = image.size(0);
    (pixels.width.0 > 0 && pixels.height.0 > 0)
        .then(|| size(pixels.width.0 as f32, pixels.height.0 as f32))
}

fn notify(state: &gpui::Entity<LightboxState>, cx: &mut App) {
    state.update(cx, |_, cx| cx.notify());
}

impl RenderOnce for ImageLightbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| {
                let focus = cx.focus_handle();
                let return_focus = window.focused(cx);
                let handle = focus.clone();
                window.defer(cx, move |window, cx| window.focus(&handle, cx));
                LightboxState {
                    view: ViewState::default(),
                    shown: SharedString::default(),
                    focus,
                    return_focus,
                }
            },
        );

        let close: CloseHandler = {
            let state = state.clone();
            let on_close = self.on_close.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                if let Some(focus) = state.update(cx, |s, _| s.return_focus.take()) {
                    window.focus(&focus, cx);
                }
                if let Some(on_close) = &on_close {
                    on_close(window, cx);
                }
            })
        };

        let shown = self.image.name.clone();
        state.update(cx, |s, _| {
            if s.shown != shown {
                let bounds = s.view.bounds;
                s.view = ViewState {
                    bounds,
                    ..ViewState::default()
                };
                s.shown = shown;
            }
        });
        let natural = self
            .image
            .natural_size
            .or_else(|| decoded_size(&self.image.source, window, cx));
        let content = match natural {
            Some(natural) => viewer(&key, &state, &self.image.source, natural, cx),
            None => div()
                .text_color(t.ink(0.6))
                .child("Loading image…")
                .into_any_element(),
        };
        let focus = state.read(cx).focus.clone();

        // A visible way out besides Escape and the backdrop: a glass capsule
        // top right (Pink audit SS-09).
        let close_button = {
            let close = close.clone();
            div()
                .id(SharedString::from(format!("{key}-close")))
                .role(Role::Button)
                .aria_label("Close image")
                .absolute()
                .top(px(CLOSE_INSET))
                .right(px(CLOSE_INSET))
                .size(px(CLOSE_SIZE))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .hover_fill(format!("{key}-close"), t.wash(0.14), t.wash(0.22))
                .cursor_pointer()
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    close(window, cx);
                })
                .child(
                    svg()
                        .path(ICON_CLOSE)
                        .size(px(CLOSE_GLYPH))
                        .flex_none()
                        .text_color(gpui::white()),
                )
        };

        let viewport = window.viewport_size();
        let max_w = viewport.width * BOX_W;
        let max_h = viewport.height * BOX_H;
        let on_key = close.clone();
        let press = state.clone();
        let click = state.clone();
        let overlay = div()
            .id(self.id.clone())
            .occlude()
            .track_focus(&focus)
            .w(viewport.width)
            .h(viewport.height)
            .font_family(pink::SANS)
            .bg(t.scrim(SCRIM))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(GAP))
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    on_key(window, cx);
                }
            })
            .capture_any_mouse_down(move |event, _, cx| {
                if event.button == MouseButton::Left {
                    press.update(cx, |s, _| s.view.begin_click());
                }
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                if !click.read(cx).view.dragged {
                    close(window, cx);
                }
            })
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .relative()
                    .w(max_w)
                    .h(max_h)
                    .child(content)
                    .with_animation(
                        SharedString::from(format!("{key}-image")),
                        LIGHTBOX_IN.animation(),
                        |el, p| el.opacity(p).top(px(RISE * (1.0 - p))),
                    ),
            )
            .child(
                div()
                    .max_w(max_w)
                    .overflow_hidden()
                    .truncate()
                    .text_size(px(TextStyle::Footnote.size()))
                    .line_height(px(TextStyle::Footnote.line_height()))
                    .text_color(t.ink(0.7))
                    .child(self.image.name.clone()),
            )
            .child(close_button)
            // The backdrop fades in rather than snapping.
            .with_animation(
                SharedString::from(format!("{key}-in")),
                LIGHTBOX_BACKDROP_IN.animation(),
                |el, p| el.opacity(p),
            );
        // Over everything else in the window, like a modal.
        deferred(anchored().position(point(px(0.0), px(0.0))).child(overlay)).priority(3)
    }
}

/// Pink `ImageView::render`: the image placed by the geometry inside a
/// clipped viewport that takes the wheel, pinch and drag.
fn viewer(
    key: &str,
    state: &gpui::Entity<LightboxState>,
    source: &ImageSource,
    natural: Size<f32>,
    cx: &mut App,
) -> AnyElement {
    let geometry = state.update(cx, |s, _| {
        let viewport = s.view.bounds.size;
        s.view.geometry.resize(
            natural,
            size(f32::from(viewport.width), f32::from(viewport.height)),
        );
        s.view.geometry
    });
    let origin = geometry.image_origin();
    let (wheel, pinch, down, up, up_out, click, measure, movement) = (
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
    );
    let viewport = div()
        .id(SharedString::from(format!("{key}-viewport")))
        .flex_1()
        .min_w_0()
        .min_h_0()
        .w_full()
        .relative()
        .overflow_hidden()
        .on_scroll_wheel(move |event, window, cx| {
            if wheel.update(cx, |s, _| s.view.wheel(event)) {
                cx.stop_propagation();
                window.prevent_default();
                notify(&wheel, cx);
            }
        })
        .on_pinch(move |event, window, cx| {
            pinch.update(cx, |s, _| s.view.pinch(event));
            cx.stop_propagation();
            window.prevent_default();
            notify(&pinch, cx);
        })
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            down.update(cx, |s, _| s.view.pointer_down(event.position));
            window.prevent_default();
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            up.update(cx, |s, _| s.view.drag = None);
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            up_out.update(cx, |s, _| s.view.drag = None);
        })
        // A drag is not a click; a plain click falls through to the
        // backdrop, which closes.
        .on_click(move |_, _, cx| {
            if click.read(cx).view.dragged {
                cx.stop_propagation();
            }
        })
        .child(
            img(source.clone())
                .absolute()
                .left(px(origin.x))
                .top(px(origin.y))
                .w(px(natural.width * geometry.scale))
                .h(px(natural.height * geometry.scale))
                .object_fit(ObjectFit::Contain),
        )
        .child(
            canvas(
                move |bounds, window, cx| {
                    let changed = measure.update(cx, |s, _| {
                        if s.view.bounds == bounds {
                            return false;
                        }
                        s.view.bounds = bounds;
                        s.view.geometry.resize(
                            natural,
                            size(f32::from(bounds.size.width), f32::from(bounds.size.height)),
                        );
                        true
                    });
                    if changed {
                        let measure = measure.clone();
                        window.defer(cx, move |_, cx| notify(&measure, cx));
                    }
                },
                move |_, _, window, _| {
                    // Continue an image drag beyond the viewport without taking
                    // over other pointers; release is handled inside and out.
                    let movement = movement.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble
                            && movement.update(cx, |s, _| s.view.pointer_move(event))
                        {
                            cx.stop_propagation();
                            notify(&movement, cx);
                        }
                    });
                },
            )
            .absolute()
            .inset_0(),
        );
    div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .child(viewport)
        .into_any_element()
}

/// Pink `transcript.rs` `render_user_attachments`: a 112×80 thumbnail above
/// a user bubble – radius 8, a `hairline(0.11)` edge on `ink(0.035)`, the
/// image covering it. Add `.on_click` to open the lightbox.
pub fn attachment_thumb(
    id: impl Into<ElementId>,
    source: impl Into<ImageSource>,
    t: &Pink,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(THUMB_W))
        .h(px(THUMB_H))
        .rounded(px(radius::MD))
        .overflow_hidden()
        .relative()
        .border_1()
        .border_color(t.hairline(0.11))
        .bg(t.ink(0.035))
        .cursor_pointer()
        .child(
            // Explicit dims: the frame's rounding clips rectangularly, so the
            // image rounds its own corners.
            img(source.into())
                .w(px(THUMB_W - 2.0))
                .h(px(THUMB_H - 2.0))
                .rounded(px(radius::MD))
                .object_fit(ObjectFit::Cover),
        )
}

/// Pink: the attachment strip above a user bubble – right-aligned and
/// wrapping, 8px apart, `px 4 pt 4 pb 6`.
pub fn attachment_strip() -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex_none()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_end()
        .items_start()
        .gap(px(8.0))
        .px(px(4.0))
        .pt(px(4.0))
        .pb(px(6.0))
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// The column the demo turn sits in.
const COLUMN_W: f32 = 560.0;
/// When the demo opens the first image by itself.
const AUTO_OPEN: Duration = Duration::from_millis(1200);

struct Demo {
    open: Option<usize>,
    auto_opened: bool,
    _auto: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let started = Instant::now();
        let auto = cx.spawn(async move |this, cx| {
            let wait = AUTO_OPEN.saturating_sub(Instant::now().saturating_duration_since(started));
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |demo, cx| {
                if !demo.auto_opened {
                    demo.auto_opened = true;
                    demo.open = Some(0);
                    cx.notify();
                }
            });
        });
        Self {
            open: None,
            auto_opened: false,
            _auto: auto,
        }
    }
}

fn demo_images() -> [LightboxImage; 2] {
    [
        LightboxImage::new("checkout-coupon-error.png", IMAGE_CHECKOUT),
        LightboxImage::new("p95-latency.png", IMAGE_LATENCY),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("image-lightbox-demo", cx, |_, cx| Demo::new(cx));
        let open = demo.read(cx).open;
        let images = demo_images();

        let thumbs = images.iter().enumerate().map(|(ix, image)| {
            let demo = demo.clone();
            attachment_thumb(
                SharedString::from(format!("image-lightbox-thumb-{ix}")),
                image.source.clone(),
                &t,
            )
            .on_click(move |_, _, cx| {
                demo.update(cx, |demo, cx| {
                    demo.auto_opened = true;
                    demo.open = Some(ix);
                    cx.notify();
                })
            })
        });

        // Pink's user bubble: right-aligned, at most 80% of the column,
        // radius 16, `px 16 py 10`, 14/22.
        let bubble = div()
            .max_w(px(COLUMN_W * 0.8))
            .bg(t.user_bubble_bg())
            .when_some(t.user_bubble_edge(), |el, edge| {
                el.shadow(vec![BoxShadow {
                    color: edge,
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(1.),
                    inset: true,
                }])
            })
            .rounded(px(radius::BUBBLE))
            .px(px(16.))
            .py(px(10.))
            .text_size(px(14.))
            .line_height(px(22.))
            .text_color(t.text)
            .child(
                "Applying SPRING-15 at checkout throws this, and p95 jumped right after \
                 Monday's deploy. Related?",
            );

        let turn = div()
            .w(px(COLUMN_W))
            .flex()
            .flex_col()
            .child(attachment_strip().children(thumbs))
            .child(div().w_full().flex().justify_end().child(bubble))
            .child(
                div()
                    .pt(px(16.))
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(t.text)
                    .child(
                        "Likely. The 500 comes from the coupon service timing out, and \
                         the deploy doubled its p95. Open either image for a closer look.",
                    ),
            );

        div()
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(turn)
            .when_some(
                open.and_then(|ix| images.get(ix).cloned()),
                |stage, image| {
                    let demo = demo.clone();
                    stage.child(ImageLightbox::new("image-lightbox", image).on_close(
                        move |_, cx| {
                            demo.update(cx, |demo, cx| {
                                demo.open = None;
                                cx.notify();
                            })
                        },
                    ))
                },
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> Geometry {
        let mut g = Geometry::default();
        g.resize(size(1000.0, 500.0), size(500.0, 300.0));
        g
    }

    #[test]
    fn fit_keeps_the_aspect_ratio_and_never_upscales() {
        let mut g = geometry();
        assert_eq!(g.scale, 0.5);
        assert_eq!(g.image_origin(), point(0.0, 25.0));
        g.resize(size(100.0, 50.0), size(500.0, 300.0));
        assert_eq!(g.scale, 1.0);
        assert_eq!(g.image_origin(), point(200.0, 125.0));
    }

    #[test]
    fn zoom_keeps_the_pointer_over_the_same_image_point() {
        let mut g = geometry();
        let anchor = point(100.0, 150.0);
        let before = (anchor - g.image_origin()) / g.scale;
        g.zoom(2.0, anchor);
        let after = (anchor - g.image_origin()) / g.scale;
        assert!((before.x - after.x).abs() < 0.001);
        assert!((before.y - after.y).abs() < 0.001);
    }

    #[test]
    fn pan_zoom_and_resize_stay_bounded() {
        let mut g = geometry();
        g.zoom(1e9, point(250.0, 150.0));
        assert_eq!(g.scale, 32.0);
        g.pan_by(point(1e9, -1e9));
        assert_eq!(g.pan, point(15750.0, -7850.0));
        g.zoom(1e-9, point(250.0, 150.0));
        assert_eq!(g.scale, 0.01);
        assert_eq!(g.pan, point(0.0, 0.0));
        g.zoom(f32::NAN, point(0.0, 0.0));
        assert_eq!(g.scale, 0.01);
        g.fit();
        g.resize(size(1000.0, 500.0), size(250.0, 200.0));
        assert_eq!(g.scale, 0.25);
        g.zoom(1.0, point(125.0, 100.0));
        g.resize(size(1000.0, 500.0), size(300.0, 200.0));
        assert_eq!(g.scale, 1.0);
    }

    #[test]
    fn wheel_zooms_only_with_control() {
        let mut state = ViewState {
            geometry: geometry(),
            ..Default::default()
        };
        let mut event = ScrollWheelEvent {
            position: point(px(250.0), px(150.0)),
            delta: ScrollDelta::Lines(point(0.0, 1.0)),
            ..Default::default()
        };
        assert!(!state.wheel(&event));
        assert_eq!(state.geometry.scale, 0.5);
        event.modifiers.control = true;
        assert!(state.wheel(&event));
        let scale = state.geometry.scale;
        assert!(scale > 0.5, "wheel up zooms in");
        state.geometry.fit();
        event.delta = ScrollDelta::Pixels(point(px(0.0), px(40.0)));
        state.wheel(&event);
        assert_eq!(state.geometry.scale, scale);
        event.delta = ScrollDelta::Lines(point(0.0, -1.0));
        state.wheel(&event);
        assert!((state.geometry.scale - 0.5).abs() < 0.0001);
    }

    #[derive(Default)]
    struct Host {
        open: bool,
        closes: usize,
    }

    impl gpui::Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let host = cx.entity().downgrade();
            div().size_full().when(self.open, |el| {
                el.child(
                    ImageLightbox::new(
                        "lb",
                        LightboxImage::new("shot.png", IMAGE_LATENCY).natural_size(1600.0, 1000.0),
                    )
                    .on_close(move |_, cx| {
                        host.update(cx, |h, cx| {
                            h.open = false;
                            h.closes += 1;
                            cx.notify();
                        })
                        .ok();
                    }),
                )
            })
        }
    }

    #[gpui::test]
    fn escape_and_a_plain_click_close_but_zoom_and_drag_do_not(cx: &mut gpui::TestAppContext) {
        use gpui::{Modifiers, MouseDownEvent, MouseUpEvent};
        cx.update(crate::init);
        let (host, cx) = cx.add_window_view(|_, _| Host {
            open: true,
            closes: 0,
        });
        let settle = |cx: &mut gpui::VisualTestContext| {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(300));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        };
        let reopen = |cx: &mut gpui::VisualTestContext| {
            host.update(cx, |h, cx| {
                h.open = true;
                cx.notify();
            });
            settle(cx);
        };
        settle(cx);
        let center = cx.update(|window, _| {
            let size = window.viewport_size();
            point(size.width / 2.0, size.height / 2.0)
        });

        // The lightbox takes focus on opening, so Escape reaches it.
        cx.simulate_keystrokes("escape");
        settle(cx);
        host.read_with(cx, |h, _| assert_eq!((h.open, h.closes), (false, 1)));

        // ⌃-scroll zooms and a drag pans; neither closes.
        reopen(cx);
        cx.simulate_mouse_move(center, None, Modifiers::none());
        cx.simulate_event(ScrollWheelEvent {
            position: center,
            delta: ScrollDelta::Pixels(point(px(0.0), px(200.0))),
            modifiers: Modifiers::control(),
            ..Default::default()
        });
        settle(cx);
        let moved = center + point(px(60.0), px(0.0));
        cx.simulate_event(MouseDownEvent {
            position: center,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        });
        cx.simulate_event(MouseMoveEvent {
            position: moved,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent {
            position: moved,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        });
        settle(cx);
        host.read_with(cx, |h, _| assert_eq!((h.open, h.closes), (true, 1)));

        // A plain click closes.
        cx.simulate_click(center, Modifiers::none());
        settle(cx);
        host.read_with(cx, |h, _| assert_eq!((h.open, h.closes), (false, 2)));
    }

    #[test]
    fn pinch_accumulates_and_a_drag_is_not_a_click() {
        let mut state = ViewState {
            geometry: geometry(),
            ..Default::default()
        };
        let mut event = PinchEvent {
            position: point(px(250.0), px(150.0)),
            phase: TouchPhase::Started,
            ..Default::default()
        };
        state.pinch(&event);
        event.phase = TouchPhase::Moved;
        event.delta = 0.2;
        state.pinch(&event);
        state.pinch(&event);
        assert!((state.geometry.scale - 0.7).abs() < 0.001);
        event.phase = TouchPhase::Ended;
        state.pinch(&event);
        assert!(state.pinch.is_none());
        state.pointer_down(point(px(200.0), px(150.0)));
        assert!(!state.pointer_move(&MouseMoveEvent {
            position: point(px(202.0), px(150.0)),
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }));
        assert!(state.pointer_move(&MouseMoveEvent {
            position: point(px(220.0), px(150.0)),
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }));
        assert!(state.dragged);
        state.begin_click();
        assert!(!state.dragged);
    }
}
