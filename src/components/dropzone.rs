//! Dropzone – drag files in (or browse) and watch them upload.
//!
//! The drop target lights up while files hover over it and takes real file
//! drops from the desktop. Every file gets a row with a colored type tile,
//! its speed and time left, and a ring that fills and turns into a check.
//! Files over the size limit or of a type that isn't accepted are refused in
//! place; a failed upload offers Retry and resumes where it stopped.
//!
//! Transfers are simulated (a steady per-file speed) so the component can be
//! shown on its own; `on_files` hands the real paths to your uploader.
//!
//! ```ignore
//! Dropzone::new("attachments")
//!     .max_size(25 * MB)
//!     .on_files(|paths, _, _| upload(paths))
//! ```

use std::{
    cell::Cell,
    f32::consts::PI,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnyView, App, Context, ElementId, Entity, ExternalPaths, FillOptions, FillRule,
    FontWeight, Hsla, IntoElement, PathBuilder, PathPromptOptions, PathStyle, Pixels, Point,
    SharedString, Task, Window, canvas, div, point, prelude::*, px, relative, svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt, GLIDE, MotionSpec},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, radius, space, text},
    story,
};

pub const NAME: &str = "Dropzone";
pub const DESCRIPTION: &str = "A drop target for files from the desktop with per-file type tiles, upload speed and time left, progress rings and retry on failure";

pub const MB: u64 = 1024 * 1024;

const ICON_UPLOAD: &str = "icons/dropzone/upload.svg";
const ICON_CHECK: &str = "icons/dropzone/check.svg";
const ICON_REMOVE: &str = "icons/dropzone/x.svg";
const ICON_RETRY: &str = "icons/dropzone/refresh.svg";
const ICON_ALERT: &str = "icons/dropzone/alert.svg";

const TICK: Duration = Duration::from_millis(50);
const GROUP: &str = "pink-dropzone";

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

#[derive(Clone, Debug, PartialEq)]
enum Status {
    Uploading,
    Done,
    Failed(SharedString),
    /// Refused before uploading (size or type); can only be removed.
    Refused(SharedString),
}

struct Upload {
    key: u64,
    file: QueuedFile,
    sent: f64,
    status: Status,
    /// Seconds-left estimate, smoothed so it doesn't flicker.
    eta: f64,
}

struct DropState {
    seeded: bool,
    uploads: Vec<Upload>,
    next_key: u64,
    ticker: Option<Task<()>>,
    browse: Option<Task<()>>,
    last_tick: Instant,
    /// Bumped on each drop, so the target flashes.
    drops: usize,
    measured: Rc<Cell<f32>>,
}

impl DropState {
    fn new() -> Self {
        Self {
            seeded: false,
            uploads: Vec::new(),
            next_key: 1,
            ticker: None,
            browse: None,
            last_tick: Instant::now(),
            drops: 0,
            measured: Rc::default(),
        }
    }

    fn add(&mut self, file: QueuedFile, rules: &Rules, cx: &mut Context<Self>) {
        let status = rules.check(&file);
        self.uploads.push(Upload {
            key: self.next_key,
            eta: file.size as f64 / file.speed.max(1.),
            file,
            sent: 0.,
            status,
        });
        self.next_key += 1;
        self.ensure_ticker(cx);
        cx.notify();
    }

    fn add_paths(&mut self, paths: &[PathBuf], rules: &Rules, cx: &mut Context<Self>) {
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            self.add(QueuedFile::new(name, size), rules, cx);
        }
        self.drops += 1;
    }

    fn retry(&mut self, key: u64, cx: &mut Context<Self>) {
        if let Some(upload) = self.uploads.iter_mut().find(|u| u.key == key) {
            upload.status = Status::Uploading;
            upload.file.fail_at = None;
            self.ensure_ticker(cx);
            cx.notify();
        }
    }

    fn remove(&mut self, key: u64, cx: &mut Context<Self>) {
        self.uploads.retain(|u| u.key != key);
        cx.notify();
    }

    fn clear_finished(&mut self, cx: &mut Context<Self>) {
        self.uploads.retain(|u| u.status == Status::Uploading);
        cx.notify();
    }

    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticker.is_some() {
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
            if upload.status != Status::Uploading {
                continue;
            }
            let size = upload.file.size.max(1) as f64;
            upload.sent = (upload.sent + upload.file.speed * dt).min(size);
            let left = (size - upload.sent) / upload.file.speed.max(1.);
            upload.eta += (left - upload.eta) * 0.2;
            if let Some(at) = upload.file.fail_at
                && upload.sent / size >= at as f64
            {
                upload.sent = size * at as f64;
                upload.status = Status::Failed("Connection lost".into());
            } else if upload.sent >= size {
                upload.status = Status::Done;
            } else {
                active = true;
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
    fn check(&self, file: &QueuedFile) -> Status {
        if file.size > self.max_size {
            return Status::Refused(
                format!("Over the {} limit", format_size(self.max_size)).into(),
            );
        }
        if let Some(accept) = &self.accept {
            let ext = extension(&file.name);
            if !accept.iter().any(|a| a.eq_ignore_ascii_case(&ext)) {
                return Status::Refused(format!(".{ext} files aren't accepted").into());
            }
        }
        Status::Uploading
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
                max_size: 25 * MB,
                accept: None,
            },
            seed: Vec::new(),
            on_files: None,
            width: px(468.),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Largest accepted file in bytes (25 MB by default).
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
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl Dropzone {
    fn build(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
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
                for file in seed {
                    s.add(file, &rules, cx);
                }
            });
        }

        let target = drop_target(&key, &state, &self.rules, self.on_files.clone(), &t, cx);

        let s = state.read(cx);
        let total: u64 = s
            .uploads
            .iter()
            .filter(|u| !matches!(u.status, Status::Refused(_)))
            .map(|u| u.file.size)
            .sum();
        let sent: f64 = s
            .uploads
            .iter()
            .filter(|u| !matches!(u.status, Status::Refused(_)))
            .map(|u| u.sent)
            .sum();
        let counted = s
            .uploads
            .iter()
            .filter(|u| !matches!(u.status, Status::Refused(_)))
            .count();
        let done = s
            .uploads
            .iter()
            .filter(|u| u.status == Status::Done)
            .count();
        let uploading = s.uploads.iter().any(|u| u.status == Status::Uploading);
        let finished = s.uploads.iter().any(|u| u.status != Status::Uploading);
        let attention = s
            .uploads
            .iter()
            .filter(|u| matches!(u.status, Status::Failed(_) | Status::Refused(_)))
            .count();
        let eta = s
            .uploads
            .iter()
            .filter(|u| u.status == Status::Uploading)
            .map(|u| u.eta)
            .fold(0., f64::max);

        let rows: Vec<_> = s
            .uploads
            .iter()
            .map(|upload| file_row(&key, upload, &state, &t))
            .collect();
        let has_rows = !rows.is_empty();

        let overall = if total == 0 {
            0.
        } else {
            (sent / total as f64) as f32
        };
        let overall = motion::follow(&format!("{key}-overall"), overall, GLIDE);

        let footer = has_rows.then(|| {
            let clear_state = state.clone();
            div()
                .flex()
                .flex_col()
                .gap(px(space::S8))
                .px(px(space::S16))
                .pt(px(space::S12))
                .pb(px(space::S12))
                .border_t_1()
                .border_color(t.border)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S8))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_muted)
                                .child(if uploading {
                                    format!(
                                        "{}% sent · about {} to go",
                                        (overall * 100.).round() as u32,
                                        format_eta(eta)
                                    )
                                } else if attention > 0 {
                                    format!(
                                        "{done} uploaded · {attention} need{} attention",
                                        if attention == 1 { "s" } else { "" }
                                    )
                                } else {
                                    format!("{done} uploaded · {}", format_size(total))
                                }),
                        )
                        .when(finished, |d| {
                            d.child(
                                pink::button(
                                    format!("{key}-clear"),
                                    ButtonKind::Ghost,
                                    ControlSize::Sm,
                                    &t,
                                )
                                .child("Clear finished")
                                .on_click(move |_, _, cx| {
                                    clear_state.update(cx, |s, cx| s.clear_finished(cx))
                                }),
                            )
                        }),
                )
                .child(
                    div()
                        .h(px(3.))
                        .w_full()
                        .rounded(px(2.))
                        .bg(t.fill(FillTier::Secondary))
                        .child(
                            div()
                                .h_full()
                                .w(relative(overall.clamp(0., 1.)))
                                .rounded(px(2.))
                                .bg(if uploading { t.accent } else { t.success }),
                        ),
                )
        });

        let list_height = s.measured.clone();
        let list = div()
            .flex()
            .flex_col()
            .px(px(space::S8))
            .pb(px(if has_rows { space::S8 } else { 0. }))
            .children(rows);
        let natural = list_height.get();
        let height = (natural > 0.).then(|| motion::follow(&format!("{key}-list"), natural, GLIDE));

        if uploading {
            window.request_animation_frame();
        }

        pink::card(&t)
            .id(self.id.clone())
            .w(self.width)
            .font_family(pink::SANS)
            .text_color(t.text)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(space::S16))
                    .pt(px(space::S16))
                    .pb(px(space::S12))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(text::CALLOUT))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_muted)
                                    .child(format!(
                                        "Up to {} per file",
                                        format_size(self.rules.max_size)
                                    )),
                            ),
                    )
                    .when(counted > 0, |d| {
                        d.child(pink::badge(
                            format!("{counted} file{}", if counted == 1 { "" } else { "s" }),
                            pink::Tone::Neutral,
                            &t,
                        ))
                    }),
            )
            .child(div().px(px(space::S16)).pb(px(space::S12)).child(target))
            .child(
                div()
                    .overflow_hidden()
                    .when_some(height, |d, h| d.h(px(h)))
                    .child(div().relative().child(list).child(measure(list_height))),
            )
            .children(footer)
            .into_any_element()
    }
}

fn drop_target(
    key: &str,
    state: &Entity<DropState>,
    rules: &Rules,
    on_files: Option<FilesHandler>,
    t: &Pink,
    cx: &App,
) -> impl IntoElement {
    let drops = state.read(cx).drops;
    let hover_key = format!("{key}-target");
    let hover = motion::hover_t(&hover_key);
    let accent = t.accent;
    let wash = t.accent_wash;
    let lit = t.accent.opacity(if t.dark { 0.9 } else { 0.7 });

    let orb = div()
        .relative()
        .size(px(44.))
        .flex()
        .items_center()
        .justify_center()
        // A sonar ring that keeps breathing outward.
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .border_1()
                .border_color(t.accent.opacity(0.35))
                .with_animation(
                    SharedString::from(format!("{key}-sonar")),
                    Animation::new(Duration::from_millis(2600)).repeat(),
                    |el, p| {
                        let e = motion::EASE_OUT.eval(p);
                        let grow = 10. * e;
                        el.top(px(-grow))
                            .left(px(-grow))
                            .right(px(-grow))
                            .bottom(px(-grow))
                            .opacity((1. - e) * 0.9)
                    },
                ),
        )
        .child(
            div()
                .size(px(44.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(t.surface_card)
                .border_1()
                .border_color(t.border)
                .shadow(t.card_shadow())
                .child(
                    svg()
                        .path(ICON_UPLOAD)
                        .size(px(18.))
                        .text_color(motion::mix(t.text_muted, t.accent, hover))
                        .group_drag_over::<ExternalPaths>(GROUP, move |s| s.text_color(accent)),
                ),
        );

    let browse_state = state.clone();
    let browse_rules = rules.clone();
    let browse_cb = on_files.clone();
    let browse = div()
        .id(SharedString::from(format!("{key}-browse")))
        .text_color(t.accent)
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(|s| s.underline())
        .child("browse")
        .on_click(move |_, window, cx| {
            let prompt = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("Upload".into()),
            });
            let rules = browse_rules.clone();
            let on_files = browse_cb.clone();
            let task = window.spawn(cx, async move |cx| {
                if let Ok(Ok(Some(paths))) = prompt.await {
                    cx.update(|window, cx| {
                        if let Some(f) = &on_files {
                            f(&paths, window, cx);
                        }
                    })
                    .ok();
                    return Some(paths);
                }
                None
            });
            let state = browse_state.clone();
            browse_state.update(cx, |s, cx| {
                s.browse = Some(cx.spawn(async move |_, cx| {
                    if let Some(paths) = task.await {
                        state.update(cx, |s, cx| s.add_paths(&paths, &rules, cx));
                    }
                }));
            });
        });

    let drop_state = state.clone();
    let drop_rules = rules.clone();
    let label_idle = div()
        .flex()
        .gap(px(4.))
        .child("Drop files here or")
        .child(browse);
    let label_over = div()
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .opacity(0.)
        .text_color(t.accent)
        .group_drag_over::<ExternalPaths>(GROUP, |s| s.opacity(1.))
        .child("Release to start uploading");

    let flash = (drops > 0).then(|| {
        div()
            .absolute()
            .inset_0()
            .rounded(px(radius::LG))
            .bg(t.accent)
            .with_animation(
                SharedString::from(format!("{key}-flash-{drops}")),
                MotionSpec::new(520, motion::EASE_OUT).animation(),
                |el, p| el.opacity(0.14 * (1. - p)),
            )
    });

    div()
        .id(SharedString::from(hover_key.clone()))
        .group(GROUP)
        .relative()
        .h(px(148.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(space::S12))
        .rounded(px(radius::LG))
        .border_1()
        .border_dashed()
        .border_color(motion::mix(t.border_strong, t.text_faint, hover))
        .bg(motion::mix(
            t.fill(FillTier::Quaternary),
            t.fill(FillTier::Tertiary),
            hover,
        ))
        .on_hover(motion::hover_listener(hover_key))
        .drag_over::<ExternalPaths>(move |s, _, _, _| s.bg(wash).border_color(lit))
        .on_drop(move |paths: &ExternalPaths, window, cx| {
            drop_state.update(cx, |s, cx| s.add_paths(paths.paths(), &drop_rules, cx));
            if let Some(f) = &on_files {
                f(paths.paths(), window, cx);
            }
        })
        .children(flash)
        .child(orb)
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.))
                .child(
                    div()
                        .relative()
                        .text_size(px(text::BODY))
                        .text_color(t.text)
                        .child(
                            label_idle.group_drag_over::<ExternalPaths>(GROUP, |s| s.opacity(0.)),
                        )
                        .child(label_over),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_faint)
                        .child(match &rules.accept {
                            Some(list) => list
                                .iter()
                                .map(|e| e.to_uppercase())
                                .collect::<Vec<_>>()
                                .join(", "),
                            None => "Documents, images, sheets and archives".into(),
                        }),
                ),
        )
}

fn file_row(key: &str, upload: &Upload, state: &Entity<DropState>, t: &Pink) -> impl IntoElement {
    let rkey = format!("{key}-file-{}", upload.key);
    let hover = motion::hover_t(&rkey);
    let ext = extension(&upload.file.name);
    let tone = type_color(&ext, t);
    let size = upload.file.size;
    let fraction = (upload.sent / size.max(1) as f64) as f32;
    let shown = motion::follow_from(
        &format!("{rkey}-ring"),
        0.,
        fraction,
        MotionSpec::new(200, motion::EASE_OUT),
    );

    let (meta, meta_color): (String, Hsla) = match &upload.status {
        Status::Uploading => (
            format!(
                "{} of {} · {}/s · {} left",
                format_size(upload.sent as u64),
                format_size(size),
                format_size(upload.file.speed as u64),
                format_eta(upload.eta)
            ),
            t.text_muted,
        ),
        Status::Done => (format!("{} · Uploaded", format_size(size)), t.text_muted),
        Status::Failed(reason) => (
            format!("{reason} at {}%", (fraction * 100.) as u32),
            t.danger,
        ),
        Status::Refused(reason) => (format!("{} · {reason}", format_size(size)), t.danger),
    };

    let tile = div()
        .size(px(34.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .bg(tone.opacity(if t.dark { 0.16 } else { 0.10 }))
        .text_size(px(9.5))
        .font_family(pink::MONO)
        .font_weight(FontWeight::BOLD)
        .text_color(tone)
        .child(if ext.is_empty() {
            "FILE".to_string()
        } else {
            ext.to_uppercase().chars().take(4).collect()
        });

    let status: gpui::AnyElement = match &upload.status {
        Status::Uploading => {
            progress_ring(shown, 20., 2.5, t.fill(FillTier::Primary), t.accent).into_any_element()
        }
        Status::Done => div()
            .size(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(t.success.opacity(if t.dark { 0.18 } else { 0.12 }))
            .child(
                svg()
                    .path(ICON_CHECK)
                    .size(px(12.))
                    .text_color(t.success)
                    .with_animation(
                        SharedString::from(format!("{rkey}-done")),
                        MotionSpec::new(320, motion::EASE_OUT_QUINT).animation(),
                        |el, p| el.size(px(4. + 8. * p)).opacity(p),
                    ),
            )
            .into_any_element(),
        Status::Failed(_) => {
            let state = state.clone();
            let id = upload.key;
            pink::button(
                format!("{rkey}-retry"),
                ButtonKind::Secondary,
                ControlSize::Sm,
                t,
            )
            .child(pink::glyph(ICON_RETRY, ControlSize::Sm).text_color(t.text_muted))
            .child("Retry")
            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.retry(id, cx)))
            .into_any_element()
        }
        Status::Refused(_) => svg()
            .path(ICON_ALERT)
            .size(px(18.))
            .text_color(t.danger)
            .into_any_element(),
    };

    let remove = {
        let state = state.clone();
        let id = upload.key;
        pink::icon_button(format!("{rkey}-remove"), ICON_REMOVE, ControlSize::Sm, t)
            .opacity(0.3 + 0.7 * hover)
            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.remove(id, cx)))
    };

    motion::rise_in(
        SharedString::from(format!("{rkey}-in")),
        motion::ROW_IN,
        6.,
        div()
            .id(SharedString::from(rkey.clone()))
            .flex()
            .items_center()
            .gap(px(space::S12))
            .h(px(54.))
            .px(px(space::S8))
            .rounded(px(radius::MD))
            .hover_fill(
                rkey.clone(),
                t.fill(FillTier::Quaternary).opacity(0.),
                t.fill(FillTier::Quaternary),
            )
            .child(tile)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if matches!(upload.status, Status::Refused(_)) {
                                t.text_muted
                            } else {
                                t.text
                            })
                            .child(upload.file.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(meta_color)
                            .child(meta),
                    ),
            )
            .child(status)
            .child(remove),
    )
}

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// One hue per family of file types.
fn type_color(ext: &str, t: &Pink) -> Hsla {
    match ext {
        "pdf" | "doc" | "docx" | "key" | "ppt" | "pptx" => t.danger,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "heic" => t.accent,
        "csv" | "xls" | "xlsx" | "numbers" | "tsv" => t.success,
        "zip" | "tar" | "gz" | "7z" | "rar" => t.warning,
        _ => t.text_muted,
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

fn format_eta(seconds: f64) -> String {
    let s = seconds.max(0.).ceil() as u64;
    if s >= 60 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{}s", s.max(1))
    }
}

/// Records its parent's height in `cell` (and asks for a frame when it changes).
fn measure(cell: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            if (cell.get() - h).abs() > 0.5 {
                cell.set(h);
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A track ring with an arc filled to `ratio`, starting at 12 o'clock.
fn progress_ring(ratio: f32, size: f32, width: f32, track: Hsla, fill: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let center = bounds.center();
            let r = size / 2. - width / 2.;
            if let Some(path) = annulus(center, r, width) {
                window.paint_path(path, track);
            }
            let ratio = ratio.clamp(0., 1.);
            if ratio >= 0.999 {
                if let Some(path) = annulus(center, r, width) {
                    window.paint_path(path, fill);
                }
            } else if ratio > 0.001
                && let Some(path) = arc(center, r, width, -PI / 2., ratio * 2. * PI)
            {
                window.paint_path(path, fill);
            }
        },
    )
    .size(px(size))
    .flex_none()
}

fn fill_builder() -> PathBuilder {
    PathBuilder::fill().with_style(PathStyle::Fill(
        FillOptions::default().with_fill_rule(FillRule::NonZero),
    ))
}

fn polar(c: Point<Pixels>, r: f32, a: f32) -> Point<Pixels> {
    point(c.x + px(r * a.cos()), c.y + px(r * a.sin()))
}

fn annulus(c: Point<Pixels>, r: f32, w: f32) -> Option<gpui::Path<Pixels>> {
    let steps = 72;
    let mut b = fill_builder();
    b.move_to(polar(c, r + w / 2., 0.));
    for i in 1..steps {
        b.line_to(polar(c, r + w / 2., 2. * PI * i as f32 / steps as f32));
    }
    b.close();
    b.move_to(polar(c, r - w / 2., 0.));
    for i in (1..steps).rev() {
        b.line_to(polar(c, r - w / 2., 2. * PI * i as f32 / steps as f32));
    }
    b.close();
    b.build().ok()
}

fn arc(c: Point<Pixels>, r: f32, w: f32, start: f32, sweep: f32) -> Option<gpui::Path<Pixels>> {
    let steps = ((sweep.abs() / (2. * PI)) * 72.).ceil().max(2.) as usize;
    let (outer, inner, half) = (r + w / 2., r - w / 2., w / 2.);
    let end = start + sweep;
    let mut b = fill_builder();
    b.move_to(polar(c, outer, start));
    for i in 1..=steps {
        b.line_to(polar(c, outer, start + sweep * i as f32 / steps as f32));
    }
    let tip = polar(c, r, end);
    for i in 1..6 {
        b.line_to(polar(tip, half, end + PI * i as f32 / 6.));
    }
    for i in (0..=steps).rev() {
        b.line_to(polar(c, inner, start + sweep * i as f32 / steps as f32));
    }
    let tail = polar(c, r, start);
    for i in 1..6 {
        b.line_to(polar(tail, half, start + PI + PI * i as f32 / 6.));
    }
    b.close();
    b.build().ok()
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        Dropzone::new("dropzone")
            .title("Contract attachments")
            .files([
                QueuedFile::new("q4-forecast.xlsx", 2_900_000).speed(2.6 * MB as f64),
                QueuedFile::new("master-services-agreement.pdf", 11_400_000).speed(1.7 * MB as f64),
                QueuedFile::new("site-photo-03.jpg", 4_200_000).speed(1.3 * MB as f64),
                QueuedFile::new("vendor-records.zip", 9_800_000)
                    .speed(2.1 * MB as f64)
                    .fail_at(0.58),
                QueuedFile::new("walkthrough.mov", 61 * MB),
            ])
    })
}
