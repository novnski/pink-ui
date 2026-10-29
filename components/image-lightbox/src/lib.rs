// PINK's image lightbox (attachments.rs `lightbox_with_size`): the
// window-wide layer that a transcript or composer thumbnail, a Markdown image
// and the Files image preview open. A dim scrim covers the window; the image
// frame, 90% of its width and 85% of its height, rises in at its centre with
// the file name under it; a 32 px glass capsule top right closes it. UI
// rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved scrim and caption
// colours, its viewer (the image placed by `ViewGeometry`, the zoom and pan
// maths kept here) and the file name; it adds the ids, the focus handle, the
// role and accessible name of the close capsule, its hover fill and glyph,
// the entrance animations (`rise` per frame) and every listener and gesture
// (Escape, a plain click, the wheel, a pinch, a drag).
// Plain `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{
    AbsoluteLength, Deferred, Div, Hsla, IntoElement, Pixels, Point, Size, anchored, deferred, div,
    point, prelude::*, px, rems, size,
};

/// The scrim's strength in the dark appearance (PINK `scrim_alpha(0.7)`;
/// the host resolves the light side).
pub const SCRIM_ALPHA_DARK: f32 = 0.7;
/// The image frame's share of the window's width and height.
pub const FRAME_WIDTH: f32 = 0.9;
pub const FRAME_HEIGHT: f32 = 0.85;
/// The gap between the frame and the caption, in px.
pub const GAP: f32 = 12.0;
/// How far the frame rises as it fades in, in px.
pub const RISE: f32 = 10.0;
/// The entrance timings: the scrim fades in (ease-out) while the frame rises
/// (expo-out). The host runs both with its own motion curves.
pub const FADE_MS: u64 = 200;
pub const RISE_MS: u64 = 260;
/// The caption's size, in pt at the 16 px default.
pub const CAPTION_TEXT: f32 = 12.0;
/// The close capsule, its inset from the top right corner and its glyph, in
/// px. Fixed: the window chrome does not scale with the Interface size.
pub const CLOSE_SIZE: f32 = 32.0;
pub const CLOSE_INSET: f32 = 16.0;
pub const CLOSE_GLYPH: f32 = 12.0;
/// The layer paints over popovers and menus, like a modal.
pub const LAYER_PRIORITY: usize = 3;
/// The viewer's largest zoom, and the bound on the zoomed image's longer
/// side (in px) that keeps layout coordinates finite for an image with
/// enormous logical dimensions.
pub const MAX_ZOOM: f32 = 32.0;
pub const MAX_EXTENT: f32 = 131_072.0;
/// The smallest zoom, unless fitting needs less.
pub const MIN_ZOOM: f32 = 0.01;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the lightbox draws, already resolved by the host.
pub struct LightboxStyle {
    /// The scrim over the window.
    pub scrim: Hsla,
    /// The file name under the image.
    pub caption: Hsla,
}

/// The image frame's size for a window: the bounds the host's viewer fits
/// the image into.
pub fn frame_size(viewport: Size<Pixels>) -> Size<Pixels> {
    size(
        px(f32::from(viewport.width) * FRAME_WIDTH),
        px(f32::from(viewport.height) * FRAME_HEIGHT),
    )
}

/// The layer: the whole window under the scrim, its frame, caption and
/// close capsule centred in a column. An anchored layer sizes to its
/// children, so it takes the window's size explicitly. The host adds the
/// id, `occlude`, the focus handle, the listeners, the [`frame`], the
/// [`caption`] and the [`close_button`], and fades it in over `FADE_MS`.
pub fn layer(viewport: Size<Pixels>, style: &LightboxStyle) -> Div {
    div()
        .w(viewport.width)
        .h(viewport.height)
        .bg(style.scrim)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(GAP))
}

/// The image frame: the host's viewer fills it. Animate it with [`rise`].
pub fn frame(viewport: Size<Pixels>) -> Div {
    let frame = frame_size(viewport);
    div().relative().w(frame.width).h(frame.height)
}

/// The frame's entrance at `progress` (0 to 1): it fades in while rising
/// `RISE` px into place.
pub fn rise<E: Styled>(frame: E, progress: f32) -> E {
    frame.opacity(progress).top(px(RISE * (1.0 - progress)))
}

/// The file name under the image, cut to one line no wider than the frame.
/// The host adds the name.
pub fn caption(viewport: Size<Pixels>, style: &LightboxStyle) -> Div {
    div()
        .max_w(frame_size(viewport).width)
        .overflow_hidden()
        .truncate()
        .text_size(ui_rems(CAPTION_TEXT))
        .text_color(style.caption)
}

/// The close capsule's frame, top right of the layer. The host adds the id,
/// the role and name, the glass hover fill, the pointer, the click and the
/// glyph (`CLOSE_GLYPH` px).
pub fn close_button() -> Div {
    div()
        .absolute()
        .top(px(CLOSE_INSET))
        .right(px(CLOSE_INSET))
        .size(px(CLOSE_SIZE))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
}

/// Places the layer over everything else in the window from its top left
/// corner, whichever element renders it.
pub fn place(layer: impl IntoElement) -> Deferred {
    deferred(anchored().position(point(px(0.0), px(0.0))).child(layer)).priority(LAYER_PRIORITY)
}

/// The viewer's zoom and pan, in viewport px: the image starts fitted inside
/// the viewport (never upscaled) and centred; a zoom keeps the image point
/// under its anchor in place; the pan is kept inside the image's overhang.
/// The host owns the gestures that drive it and places the image at
/// [`ViewGeometry::image_origin`], `natural * scale` in size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewGeometry {
    /// The image's logical size.
    pub natural: Size<f32>,
    /// The viewer's size.
    pub viewport: Size<f32>,
    pub scale: f32,
    /// The image centre's offset from the viewport's centre.
    pub pan: Point<f32>,
    /// Whether the image still follows the viewport's fit.
    pub fitted: bool,
}

impl Default for ViewGeometry {
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

impl ViewGeometry {
    /// The scale that fits the image inside the viewport, never above 1.
    pub fn fit_scale(&self) -> f32 {
        (self.viewport.width / self.natural.width)
            .min(self.viewport.height / self.natural.height)
            .min(1.0)
    }

    /// Takes a new image or viewport size: a fitted image fits again, a
    /// zoomed one keeps its scale with its pan clamped. Non-finite or empty
    /// sizes are ignored.
    pub fn resize(&mut self, natural: Size<f32>, viewport: Size<f32>) {
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

    /// Back to the fitted, centred image.
    pub fn fit(&mut self) {
        self.scale = self.fit_scale();
        self.pan = point(0.0, 0.0);
        self.fitted = true;
    }

    /// Keeps the pan inside the image's overhang past the viewport.
    pub fn clamp_pan(&mut self) {
        let limit = point(
            ((self.natural.width * self.scale - self.viewport.width) / 2.0).max(0.0),
            ((self.natural.height * self.scale - self.viewport.height) / 2.0).max(0.0),
        );
        self.pan.x = self.pan.x.clamp(-limit.x, limit.x);
        self.pan.y = self.pan.y.clamp(-limit.y, limit.y);
    }

    /// Zooms to `scale`, between `MIN_ZOOM` (or the fit, if smaller) and
    /// `MAX_ZOOM` (or less for a huge image), keeping the image point under
    /// `anchor` (viewport px) in place. Non-finite input is ignored.
    pub fn zoom(&mut self, scale: f32, anchor: Point<f32>) {
        if !scale.is_finite() || scale <= 0.0 || !anchor.x.is_finite() || !anchor.y.is_finite() {
            return;
        }
        let maximum = (MAX_EXTENT / self.natural.width.max(self.natural.height))
            .min(MAX_ZOOM)
            .max(self.fit_scale());
        let scale = scale.clamp(self.fit_scale().min(MIN_ZOOM), maximum);
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

    /// Pans by `delta` within the overhang; true when the image moved.
    pub fn pan_by(&mut self, delta: Point<f32>) -> bool {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return false;
        }
        let before = self.pan;
        self.pan += delta;
        self.clamp_pan();
        self.pan != before
    }

    /// The image's top left corner in the viewport.
    pub fn image_origin(&self) -> Point<f32> {
        point(
            (self.viewport.width - self.natural.width * self.scale) / 2.0 + self.pan.x,
            (self.viewport.height - self.natural.height * self.scale) / 2.0 + self.pan.y,
        )
    }

    /// Whether `local` (viewport px) falls on the image, edges included.
    pub fn contains(&self, local: Point<f32>) -> bool {
        let origin = self.image_origin();
        local.x >= origin.x
            && local.y >= origin.y
            && local.x <= origin.x + self.natural.width * self.scale
            && local.y <= origin.y + self.natural.height * self.scale
    }
}
