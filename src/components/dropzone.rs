//! Dropzone – files dropped from the desktop (or added with the button)
//! upload as thumbnails.
//!
//! Drawn the way PINK takes attachments. At rest the card shows its staged
//! files and an "Add files" ghost button (Pink's brief image editor); while
//! files hover over it a "Drop to attach" veil fades in over the whole card
//! (Pink's chat dropzone). Each file is a 112×80 thumbnail with its type's
//! artwork: a pulsing skeleton while it is read, the radial upload ring over a
//! darkened plate while its bytes go out, the dashed "missing" thumb with
//! Retry when the transfer stops, and the same spinner Pink shows during a
//! retry backoff. Files over the size limit or of a type that isn't accepted
//! never join the strip; Pink's error strip says why.
//!
//! Transfers are simulated (a steady per-file speed) so the component can be
//! shown on its own; `on_files` hands the real paths to your uploader.
//!
//! ```ignore
//! Dropzone::new("attachments")
//!     .max_size(24 * MB)
//!     .on_files(|paths, _, _| upload(paths))
//! ```

use std::{
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Context, Div, ElementId, Entity, ExternalPaths, FontWeight,
    IntoElement, PathPromptOptions, Pixels, SharedString, Task, Window, div, hsla, img, prelude::*,
    px, svg,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink, icon_size, layout, loaders, radius, space, text},
    story,
};

pub const NAME: &str = "Dropzone";
pub const DESCRIPTION: &str = "An attachments card that takes files dropped from the desktop – a Drop to attach veil while they hover, thumbnails that pulse while read, upload under a radial ring and offer Retry when a transfer stops, an Add files button, and a red strip naming any file over the size limit";

pub const MB: u64 = 1024 * 1024;

const ICON_PLUS: &str = "icons/dropzone/plus.svg";
const ICON_CLOSE: &str = "icons/dropzone/close.svg";
const ICON_DANGER: &str = "icons/dropzone/danger-circle.svg";
const ICON_WARNING: &str = "icons/dropzone/danger-triangle.svg";
/// Pink: `file_icons` – the file-type artwork (vscode-symbols); the slate
/// ones have dark variants lifted like Pink's `dark_icon_svg`.
const FILE_PDF: &str = "icons/dropzone/file-pdf.svg";
const FILE_SHEET: &str = "icons/dropzone/file-csv.svg";
const FILE_IMAGE: &str = "icons/dropzone/file-image.svg";
const FILE_ARCHIVE: &str = "icons/dropzone/file-compressed.svg";
const FILE_VIDEO: &str = "icons/dropzone/file-video.svg";
const FILE_VIDEO_DARK: &str = "icons/dropzone/file-video-dark.svg";
const FILE_DOCUMENT: &str = "icons/dropzone/file-document.svg";
const FILE_DOCUMENT_DARK: &str = "icons/dropzone/file-document-dark.svg";

/// Pink: `attachments::MAX_ATTACHMENT_BYTES`.
const DEFAULT_MAX_SIZE: u64 = 24 * MB;
/// Pink: `transcript::ATT_THUMB_W` / `ATT_THUMB_H` (also the brief editor's
/// `THUMB_W` / `THUMB_H`).
const THUMB_W: f32 = 112.;
const THUMB_H: f32 = 80.;
/// Pink: the sending overlay's `upload_progress_ring(pct, 34.0)`.
const RING: f32 = 34.;
/// Pink: the indeterminate sending spinner's cell (`mini_glyph_spinner(.., 3.0, ..)`).
const SPINNER_CELL: f32 = 3.;
/// The file artwork at its own 24 box.
const FILE_ICON: f32 = 24.;
/// Pink: `widgets::error_strip` copy size.
const STRIP_TEXT: f32 = 12.5;

const TICK: Duration = Duration::from_millis(50);
/// How long a new file reads before its bytes go out (the skeleton).
const READ_SECS: f64 = 0.6;
/// The backoff after Retry before the transfer resumes (the spinner).
const RETRY_SECS: f64 = 1.2;

type FilesHandler = Rc<dyn Fn(&[PathBuf], &mut Window, &mut App)>;

/// A file waiting in (or seeded into) the queue.
#[derive(Clone, Debug)]
pub struct QueuedFile {
    pub name: SharedString,
    pub size: u64,
    /// Bytes per second for the simulated transfer.
    pub speed: f64,
    /// Fails once at this fraction (0..1) – for showing the retry path.
    pub fail_at: Option<f32>,
}

impl QueuedFile {
    pub fn new(name: impl Into<SharedString>, size: u64) -> Self {
        let name = name.into();
        let speed = simulated_speed(&name);
        Self {
            name,
            size,
            speed,
            fail_at: None,
        }
    }

    pub fn speed(mut self, bytes_per_second: f64) -> Self {
        self.speed = bytes_per_second;
        self
    }

    pub fn fail_at(mut self, fraction: f32) -> Self {
        self.fail_at = Some(fraction.clamp(0., 1.));
        self
    }
}

/// A plausible, stable speed per name: 1.4–3.8 MB/s.
fn simulated_speed(name: &str) -> f64 {
    let hash = name
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    (1.4 + (hash % 1000) as f64 / 1000. * 2.4) * MB as f64
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Status {
    /// Being read before upload (Pink: `AttachmentSnapshot::Loading`).
    Reading {
        wait: f64,
    },
    /// Bytes going out (Pink: the sending overlay with a percent).
    Uploading,
    /// Backing off after Retry (Pink: the sending overlay without a percent).
    Retrying {
        wait: f64,
    },
    Done,
    /// Stopped part-way (Pink: `AttachmentSnapshot::Error`).
    Failed,
}

impl Status {
    fn live(self) -> bool {
        matches!(
            self,
            Status::Reading { .. } | Status::Uploading | Status::Retrying { .. }
        )
    }
}

struct Upload {
    key: u64,
    file: QueuedFile,
    sent: f64,
    status: Status,
    /// Seconds-left estimate, smoothed so it doesn't flicker.
    eta: f64,
}

impl Upload {
    fn fraction(&self) -> f32 {
        (self.sent / self.file.size.max(1) as f64) as f32
    }
}

struct DropState {
    seeded: bool,
    uploads: Vec<Upload>,
    next_key: u64,
    /// The last batch's refusals, in Pink's copy.
    notice: Option<SharedString>,
    ticker: Option<Task<()>>,
    browse: Option<Task<()>>,
    last_tick: Instant,
}

impl DropState {
    fn new() -> Self {
        Self {
            seeded: false,
            uploads: Vec::new(),
            next_key: 1,
            notice: None,
            ticker: None,
            browse: None,
            last_tick: Instant::now(),
        }
    }

    /// Stages a batch; anything refused goes to the error strip instead
    /// (Pink: `add_brief_images` setting `edit.error`).
    fn add(&mut self, files: Vec<QueuedFile>, rules: &Rules, cx: &mut Context<Self>) {
        let mut refused = Vec::new();
        for file in files {
            if let Err(message) = rules.check(&file) {
                refused.push(message);
                continue;
            }
            self.uploads.push(Upload {
                key: self.next_key,
                eta: READ_SECS + file.size as f64 / file.speed.max(1.),
                file,
                sent: 0.,
                status: Status::Reading { wait: READ_SECS },
            });
            self.next_key += 1;
        }
        self.notice = (!refused.is_empty()).then(|| refused.join(" ").into());
        self.ensure_ticker(cx);
        cx.notify();
    }

    fn add_paths(&mut self, paths: &[PathBuf], rules: &Rules, cx: &mut Context<Self>) {
        let files = paths
            .iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                QueuedFile::new(name, size)
            })
            .collect();
        self.add(files, rules, cx);
    }

    fn retry(&mut self, key: u64, cx: &mut Context<Self>) {
        if let Some(upload) = self.uploads.iter_mut().find(|u| u.key == key) {
            upload.status = Status::Retrying { wait: RETRY_SECS };
            upload.file.fail_at = None;
            self.ensure_ticker(cx);
            cx.notify();
        }
    }

    fn remove(&mut self, key: u64, cx: &mut Context<Self>) {
        self.uploads.retain(|u| u.key != key);
        cx.notify();
    }

    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticker.is_some() || !self.uploads.iter().any(|u| u.status.live()) {
            return;
        }
        self.last_tick = Instant::now();
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let more = this.update(cx, |s, cx| s.tick(cx)).unwrap_or(false);
                if !more {
                    this.update(cx, |s, _| s.ticker = None).ok();
                    break;
                }
            }
        }));
    }

    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64() / motion::speed_scale() as f64;
        self.last_tick = now;
        let mut active = false;
        for upload in &mut self.uploads {
            let speed = upload.file.speed.max(1.);
            let size = upload.file.size.max(1) as f64;
            match upload.status {
                Status::Reading { wait } | Status::Retrying { wait } => {
                    let wait = wait - dt;
                    upload.status = match upload.status {
                        _ if wait <= 0. => Status::Uploading,
                        Status::Reading { .. } => Status::Reading { wait },
                        _ => Status::Retrying { wait },
                    };
                    let left = wait.max(0.) + (size - upload.sent) / speed;
                    upload.eta += (left - upload.eta) * 0.2;
                    active = true;
                }
                Status::Uploading => {
                    upload.sent = (upload.sent + upload.file.speed * dt).min(size);
                    let left = (size - upload.sent) / speed;
                    upload.eta += (left - upload.eta) * 0.2;
                    if let Some(at) = upload.file.fail_at
                        && upload.sent / size >= at as f64
                    {
                        upload.sent = size * at as f64;
                        upload.status = Status::Failed;
                    } else if upload.sent >= size {
                        upload.status = Status::Done;
                    } else {
                        active = true;
                    }
                }
                Status::Done | Status::Failed => {}
            }
        }
        cx.notify();
        active
    }
}

/// What the dropzone accepts.
#[derive(Clone)]
struct Rules {
    max_size: u64,
    accept: Option<Vec<SharedString>>,
}

impl Rules {
    /// Pink: `attachments::stage_file` – the refusal copy names the file.
    fn check(&self, file: &QueuedFile) -> Result<(), String> {
        if let Some(accept) = &self.accept {
            let ext = extension(&file.name);
            if !accept.iter().any(|a| a.eq_ignore_ascii_case(&ext)) {
                return Err(format!("{} is not a supported file type.", file.name));
            }
        }
        if file.size > self.max_size {
            return Err(format!(
                "{} is too large ({} max).",
                file.name,
                format_size(self.max_size)
            ));
        }
        Ok(())
    }

    /// The limits sentence under the strip.
    fn limits(&self) -> String {
        let size = format_size(self.max_size);
        match &self.accept {
            Some(list) => {
                let names: Vec<String> = list.iter().map(|e| e.to_uppercase()).collect();
                let types = match names.as_slice() {
                    [] => String::new(),
                    [only] => only.clone(),
                    [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
                };
                format!("{types} files, up to {size} each.")
            }
            None => format!("Up to {size} per file."),
        }
    }
}

#[derive(IntoElement)]
pub struct Dropzone {
    id: ElementId,
    title: SharedString,
    rules: Rules,
    seed: Vec<QueuedFile>,
    on_files: Option<FilesHandler>,
    width: Pixels,
}

impl Dropzone {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: "Attachments".into(),
            rules: Rules {
                max_size: DEFAULT_MAX_SIZE,
                accept: None,
            },
            seed: Vec::new(),
            on_files: None,
            width: px(600.),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Largest accepted file in bytes (24 MB by default, as in Pink).
    pub fn max_size(mut self, bytes: u64) -> Self {
        self.rules.max_size = bytes;
        self
    }

    /// Accepted extensions, without the dot. Everything goes when unset.
    pub fn accept(mut self, extensions: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.rules.accept = Some(extensions.into_iter().map(Into::into).collect());
        self
    }

    /// Files already in the queue when the dropzone first renders.
    pub fn files(mut self, files: impl IntoIterator<Item = QueuedFile>) -> Self {
        self.seed.extend(files);
        self
    }

    /// Called with the paths of every drop or browse.
    pub fn on_files(
        mut self,
        handler: impl Fn(&[PathBuf], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_files = Some(Rc::new(handler));
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }
}

impl RenderOnce for Dropzone {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let element = self.build(window, cx);
        // Hover fades started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl Dropzone {
    /// Pink: `BoardDetail::render_brief` (a section card holding the brief
    /// editor) with `Shell::render_main`'s attachment veil over it.
    fn build(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| DropState::new(),
        );
        if !state.read(cx).seeded {
            let (seed, rules) = (self.seed.clone(), self.rules.clone());
            state.update(cx, |s, cx| {
                s.seeded = true;
                if !seed.is_empty() {
                    s.add(seed, &rules, cx);
                }
            });
        }

        // Pink: the skeleton and the sending plate breathe on the shared
        // pulse clock; only lease it while something is in flight.
        let live = state.read(cx).uploads.iter().any(|u| u.status.live());
        let pulse = if live {
            motion::pulse_wave(motion::pulse_phase(&motion::PINK_PULSE, window, cx))
        } else {
            0.
        };

        let s = state.read(cx);
        let summary = summary(&s.uploads);
        let tiles: Vec<AnyElement> = s
            .uploads
            .iter()
            .map(|upload| thumbnail(&key, upload, &state, pulse, &t).into_any_element())
            .collect();
        let empty = tiles.is_empty();
        let notice = s.notice.clone();
        let browsing = s.browse.is_some();

        let add = add_button(
            &key,
            browsing,
            &state,
            &self.rules,
            self.on_files.clone(),
            &t,
        );

        // Pink: `render_brief_image_editor` – the wrapping strip with Add
        // last, then the hint.
        let image_editor = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(space::S6))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_start()
                    .gap(px(space::S8))
                    .children(tiles)
                    // The button centres on the thumbnails, not their captions.
                    .child(
                        div()
                            .when(!empty, |slot| slot.h(px(THUMB_H)))
                            .flex()
                            .items_center()
                            .child(add),
                    ),
            )
            .child(pink::row_description(
                format!(
                    "Drop files on this card or add them here; they upload right away. {}",
                    self.rules.limits()
                ),
                &t,
            ));

        let editor = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(space::S8))
            .child(image_editor)
            .children(notice.map(|message| error_strip(message, &t)));

        let drop_state = state.clone();
        let drop_rules = self.rules.clone();
        let on_files = self.on_files.clone();

        pink::card(&t)
            .id(self.id.clone())
            .relative()
            .w(self.width)
            .overflow_hidden()
            .flex()
            .flex_col()
            .text_color(t.text)
            .on_drop(move |paths: &ExternalPaths, window, cx| {
                cx.stop_propagation();
                drop_state.update(cx, |s, cx| s.add_paths(paths.paths(), &drop_rules, cx));
                if let Some(f) = &on_files {
                    f(paths.paths(), window, cx);
                }
            })
            .child(
                pink::section_header(self.title.clone(), Some(summary.into()), &t)
                    .font_features(pink::tabular_figures()),
            )
            .child(pink::card_row(false, &t).child(editor))
            .child(drop_veil(&t))
            .into_any_element()
    }
}

/// The header line: progress while files move, then the tally.
fn summary(uploads: &[Upload]) -> String {
    let total = uploads.len();
    let done = uploads.iter().filter(|u| u.status == Status::Done).count();
    let failed = uploads
        .iter()
        .filter(|u| u.status == Status::Failed)
        .count();
    let live: Vec<&Upload> = uploads.iter().filter(|u| u.status.live()).collect();
    if total == 0 {
        "No files yet".into()
    } else if !live.is_empty() {
        let eta = live.iter().map(|u| u.eta).fold(0., f64::max);
        format!(
            "{done} of {total} uploaded · about {} left",
            loaders::format_elapsed(eta.ceil().max(1.) as i64)
        )
    } else if failed > 0 {
        format!("{done} uploaded · {failed} failed")
    } else {
        let bytes: u64 = uploads.iter().map(|u| u.file.size).sum();
        format!(
            "{total} file{} · {}",
            if total == 1 { "" } else { "s" },
            format_size(bytes)
        )
    }
}

/// Pink: `shell.rs` "attachment-drop-overlay" – hidden until files hover,
/// then a 0.4 (dark) scrim with a centred 13 pt "Drop to attach". GPUI
/// matches the style against the payload type, so only file drags reveal it.
fn drop_veil(t: &Pink) -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius::inner(radius::SECTION_CARD, 1.)))
        .opacity(0.)
        .bg(t
            .scrim(pink::SCRIM_ALPHA_DARK)
            .opacity(0.4 / pink::SCRIM_ALPHA_DARK))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(text::BODY))
        .text_color(t.text)
        .drag_over::<ExternalPaths>(|style, _, _, _| style.opacity(1.))
        .child("Drop to attach")
}

/// Pink: the brief editor's "Add image" – a small ghost button, label then
/// a plus, disabled while the picker is open.
fn add_button(
    key: &str,
    browsing: bool,
    state: &Entity<DropState>,
    rules: &Rules,
    on_files: Option<FilesHandler>,
    t: &Pink,
) -> impl IntoElement {
    let state = state.clone();
    let rules = rules.clone();
    pink::button_enabled(
        format!("{key}-add"),
        ButtonKind::Ghost,
        ControlSize::Sm,
        !browsing,
        t,
    )
    .aria_label("Add files")
    .child("Add files")
    .child(pink::glyph(ICON_PLUS, ControlSize::Sm))
    .when(!browsing, |button| {
        button.on_click(move |_, window, cx| {
            let prompt = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("Add".into()),
            });
            let rules = rules.clone();
            let on_files = on_files.clone();
            state.update(cx, |s, cx| {
                s.browse = Some(cx.spawn_in(window, async move |this, cx| {
                    let paths = match prompt.await {
                        Ok(Ok(Some(paths))) => Some(paths),
                        _ => None,
                    };
                    this.update(cx, |s, cx| {
                        s.browse = None;
                        match &paths {
                            Some(paths) => s.add_paths(paths, &rules, cx),
                            None => cx.notify(),
                        }
                    })
                    .ok();
                    if let (Some(paths), Some(f)) = (paths, on_files) {
                        cx.update(|window, cx| f(&paths, window, cx)).ok();
                    }
                }));
                cx.notify();
            });
        })
    })
}

/// One staged file: Pink's 112×80 attachment thumbnail (transcript
/// `render_user_attachments` states, brief-editor Remove) with a caption.
fn thumbnail(
    key: &str,
    upload: &Upload,
    state: &Entity<DropState>,
    pulse: f32,
    t: &Pink,
) -> impl IntoElement {
    let tkey = format!("{key}-file-{}", upload.key);
    let frame = div()
        .size_full()
        .rounded(px(radius::MD))
        .overflow_hidden()
        .border_1();

    let frame = match upload.status {
        // Pink: the loading skeleton (the popover skeletons' wash and pulse).
        Status::Reading { .. } => frame
            .border_color(t.hairline(0.08))
            .bg(t.ink(0.055))
            .opacity(0.35 + 0.4 * pulse),
        // Pink: the dashed "missing" thumb, holding the inline Retry.
        Status::Failed => {
            let state = state.clone();
            let id = upload.key;
            frame
                .border_dashed()
                .border_color(t.hairline(0.14))
                .bg(t.ink(0.025))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    pink::button(
                        format!("{tkey}-retry"),
                        ButtonKind::Secondary,
                        ControlSize::Sm,
                        t,
                    )
                    .aria_label(format!("Retry {}", upload.file.name))
                    .child("Retry")
                    .on_click(move |_, _, cx| state.update(cx, |s, cx| s.retry(id, cx))),
                )
        }
        status => {
            let plate = frame
                .relative()
                .border_color(t.hairline(0.11))
                .bg(t.ink(0.035))
                .flex()
                .items_center()
                .justify_center();
            let indicator: Option<AnyElement> = match status {
                Status::Uploading => Some(loaders::upload_progress_ring(
                    (upload.fraction() * 100.).floor().clamp(0., 100.) as u8,
                    RING,
                )),
                Status::Retrying { .. } => Some(
                    loaders::mini_glyph_spinner(format!("{tkey}-spin"), SPINNER_CELL, t.glyph)
                        .into_any_element(),
                ),
                _ => None,
            };
            // The artwork stands in for Pink's photo; while bytes move the
            // ring takes its place so the percent stays legible.
            let artwork = indicator.is_none().then(|| {
                img(file_icon(&upload.file.name, t.dark))
                    .flex_none()
                    .size(px(FILE_ICON))
            });
            // Pink: the sending overlay – the plate dimmed to black 38%,
            // breathing 5% on the pulse, a fixed white ring on top.
            plate.children(artwork).children(indicator.map(|indicator| {
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(radius::MD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(hsla(0., 0., 0., 0.38 + 0.05 * pulse))
                    .child(indicator)
            }))
        }
    };

    // Pink: the brief editor's Remove – a small icon button on a near-opaque
    // disc in the corner, always there.
    let remove = {
        let state = state.clone();
        let id = upload.key;
        pink::icon_button(format!("{tkey}-remove"), ICON_CLOSE, ControlSize::Sm, t)
            .aria_label(format!("Remove {}", upload.file.name))
            .absolute()
            .top(px(space::S4))
            .right(px(space::S4))
            .rounded_full()
            .bg(t.bg.opacity(0.85))
            .tooltip(pink::text_tooltip("Remove file"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                state.update(cx, |s, cx| s.remove(id, cx));
            })
    };

    let meta = match upload.status {
        Status::Reading { .. } => div().text_color(t.content_muted).child("Reading…"),
        Status::Uploading => div().text_color(t.content_muted).child(format!(
            "{} left",
            loaders::format_elapsed(upload.eta.ceil().max(1.) as i64)
        )),
        Status::Retrying { .. } => div().text_color(t.content_muted).child("Reconnecting…"),
        Status::Done => div()
            .text_color(t.content_muted)
            .child(format_size(upload.file.size)),
        // Pink: one red glyph plus one red word.
        Status::Failed => div()
            .flex()
            .items_center()
            .gap(px(space::S4))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.readable_tone(t.danger, 0.9))
            .child(
                svg()
                    .path(ICON_DANGER)
                    .flex_none()
                    .size(px(icon_size::XS))
                    .text_color(t.readable_icon(t.danger)),
            )
            .child(format!("Failed at {}%", (upload.fraction() * 100.) as u32)),
    };

    motion::rise_in(
        SharedString::from(format!("{tkey}-in")),
        motion::ATTACHMENT_IN,
        8.,
        div()
            .w(px(THUMB_W))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(space::S6))
            .child(
                div()
                    .relative()
                    .w(px(THUMB_W))
                    .h(px(THUMB_H))
                    .child(frame)
                    .child(remove),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(space::TEXT_STACK_GAP))
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_center()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(t.text)
                            .child(upload.file.name.clone()),
                    )
                    .child(
                        meta.text_size(px(text::CAPTION))
                            .line_height(px(14.))
                            .font_features(pink::tabular_figures()),
                    ),
            ),
    )
}

/// Pink: `widgets::error_strip` – the danger plate with a leading triangle.
fn error_strip(message: SharedString, t: &Pink) -> Div {
    let (plate, edge, copy) = pink::strip_colors(t.danger, t.danger_muted, t);
    div()
        .px(px(layout::ROW_PAD_X))
        .py(px(layout::ROW_PAD_Y))
        .rounded(px(radius::STRIP))
        .border_1()
        .border_color(edge)
        .bg(plate)
        .text_size(px(STRIP_TEXT))
        .text_color(copy)
        .flex()
        .flex_row()
        .items_start()
        .gap(px(space::S8))
        .child(
            div().flex_none().mt(px(space::S2)).child(
                svg()
                    .path(ICON_WARNING)
                    .size(px(icon_size::MD))
                    .text_color(copy),
            ),
        )
        .child(div().min_w_0().child(message))
}

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Pink: `file_icons::asset_path` for the kinds a dropzone sees.
fn file_icon(name: &str, dark: bool) -> &'static str {
    match extension(name).as_str() {
        "pdf" => FILE_PDF,
        "csv" | "tsv" | "xls" | "xlsx" => FILE_SHEET,
        "png" | "jpg" | "jpeg" | "heic" | "webp" => FILE_IMAGE,
        "zip" | "tar" | "gz" | "7z" | "rar" => FILE_ARCHIVE,
        "mov" | "mp4" | "m4v" | "webm" | "avi" if dark => FILE_VIDEO_DARK,
        "mov" | "mp4" | "m4v" | "webm" | "avi" => FILE_VIDEO,
        _ if dark => FILE_DOCUMENT_DARK,
        _ => FILE_DOCUMENT,
    }
}

fn format_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= MB as f64 {
        let mb = b / MB as f64;
        if mb >= 9.95 {
            format!("{mb:.0} MB")
        } else {
            format!("{mb:.1} MB")
        }
    } else if b >= 1024. {
        format!("{:.0} KB", b / 1024.)
    } else {
        format!("{bytes} B")
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        Dropzone::new("dropzone")
            .title("Contract attachments")
            .files([
                QueuedFile::new("q4-forecast.xlsx", 2_900_000).speed(2.6 * MB as f64),
                QueuedFile::new("master-services-agreement.pdf", 11_400_000).speed(1.2 * MB as f64),
                QueuedFile::new("site-photo-03.jpg", 4_200_000).speed(1.3 * MB as f64),
                QueuedFile::new("vendor-records.zip", 9_800_000)
                    .speed(2.1 * MB as f64)
                    .fail_at(0.58),
                QueuedFile::new("walkthrough.mov", 61 * MB),
            ])
    })
}
