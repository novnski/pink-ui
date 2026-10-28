//! The component gallery: every pink-ui component with its demo.
//!
//! ```text
//! cargo run                                   # browse everything
//! cargo run -- profile-menu --theme dark      # open one component
//! cargo run -- profile-menu --stage-only --size 894x520
//! cargo run -- sidebar --ui-size 20             # Pink's Interface size, 12 to 20
//! ```
//!
//! `--stage-only` renders just the preview stage, which is what
//! `scripts/screenshot.sh` captures.

use gpui::{
    AnyView, App, Bounds, Context, FontWeight, Hsla, KeyBinding, ScrollHandle, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, point, prelude::*, px,
    size,
};
use pink_ui::{
    ActiveTheme, Assets, HoverFill, Theme, ThemeMode,
    components::{CATEGORIES, COMPONENTS},
    pink::Pink,
    primitives::{SANS, hex, hexa},
};

actions!(gallery, [Next, Previous, ToggleTheme, Replay, Quit]);

struct Args {
    slug: Option<String>,
    mode: ThemeMode,
    stage_only: bool,
    size: Option<(f32, f32)>,
    ui_size: Option<f32>,
}

fn parse_args() -> Args {
    let mut args = Args {
        slug: None,
        mode: ThemeMode::Light,
        stage_only: false,
        size: None,
        ui_size: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--theme" => {
                if it.next().as_deref() == Some("dark") {
                    args.mode = ThemeMode::Dark;
                }
            }
            "--dark" => args.mode = ThemeMode::Dark,
            "--stage-only" => args.stage_only = true,
            "--component" => args.slug = it.next(),
            "--size" => {
                args.size = it.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
            }
            "--ui-size" => args.ui_size = it.next().and_then(|s| s.parse().ok()),
            "--list" => {
                for c in COMPONENTS {
                    println!("{:<22} {:<14} {}", c.slug, c.category, c.name);
                }
                std::process::exit(0);
            }
            other if !other.starts_with('-') => args.slug = Some(other.to_string()),
            other => eprintln!("gallery: ignoring unknown flag {other}"),
        }
    }
    args
}

/// The gallery chrome colors, matching the showcase site's.
struct Chrome {
    /// pink-ui's brand pink.
    brand: Hsla,
    bg: Hsla,
    preview: Hsla,
    text: Hsla,
    muted: Hsla,
    border: Hsla,
    border_soft: Hsla,
    hover: Hsla,
    selected: Hsla,
}

impl Chrome {
    fn get(cx: &App) -> Self {
        if cx.is_dark() {
            Chrome {
                brand: hex(0xf472b6),
                bg: hex(0x0c0c0d),
                // Pink's content panel, so the stage around a demo is the app's.
                preview: Pink::dark().bg,
                text: hex(0xededed),
                muted: hex(0x8a8a8f),
                border: hex(0x262628),
                border_soft: hexa(0xffffff, 0.07),
                hover: hexa(0xffffff, 0.05),
                selected: hexa(0xffffff, 0.09),
            }
        } else {
            Chrome {
                brand: hex(0xdb2777),
                bg: hex(0xf5f5f5),
                preview: Pink::light().bg,
                text: hex(0x0a0a0a),
                muted: hex(0x737373),
                border: hex(0xe5e5e5),
                border_soft: hexa(0x0a0a0a, 0.1),
                hover: hexa(0x000000, 0.04),
                selected: hexa(0x000000, 0.07),
            }
        }
    }
}

struct Gallery {
    selected: usize,
    story: AnyView,
    stage_only: bool,
    sidebar_scroll: ScrollHandle,
}

impl Gallery {
    fn new(selected: usize, stage_only: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let story = (COMPONENTS[selected].story)(window, cx);
        Gallery {
            selected,
            story,
            stage_only,
            sidebar_scroll: ScrollHandle::new(),
        }
    }

    fn select(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = ix;
        self.story = (COMPONENTS[ix].story)(window, cx);
        window.set_window_title(&format!("{} in pink-ui", COMPONENTS[ix].name));
        cx.notify();
    }

    fn next(&mut self, _: &Next, window: &mut Window, cx: &mut Context<Self>) {
        self.select((self.selected + 1) % COMPONENTS.len(), window, cx);
    }

    fn previous(&mut self, _: &Previous, window: &mut Window, cx: &mut Context<Self>) {
        self.select(
            (self.selected + COMPONENTS.len() - 1) % COMPONENTS.len(),
            window,
            cx,
        );
    }

    fn replay(&mut self, _: &Replay, window: &mut Window, cx: &mut Context<Self>) {
        self.select(self.selected, window, cx);
    }

    fn toggle_theme(&mut self, _: &ToggleTheme, _: &mut Window, cx: &mut Context<Self>) {
        let mode = cx.theme().mode.toggled();
        Theme::set_mode(mode, cx);
    }

    fn stage(&self, chrome: &Chrome) -> impl IntoElement {
        div()
            .id("stage")
            .relative()
            .flex()
            .flex_1()
            .size_full()
            .items_center()
            .justify_center()
            .p(px(45.))
            .rounded(px(18.))
            .overflow_hidden()
            .bg(chrome.preview)
            .border_1()
            .border_color(chrome.border_soft)
            .child(self.story.clone())
    }

    fn sidebar(&self, chrome: &Chrome, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div()
            .id("sidebar-list")
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(2.))
            .px(px(10.))
            .pb(px(16.))
            .overflow_y_scroll()
            .track_scroll(&self.sidebar_scroll);

        for category in CATEGORIES {
            list = list.child(
                div()
                    .pt(px(14.))
                    .pb(px(4.))
                    .px(px(8.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(chrome.muted)
                    .child(*category),
            );
            for (ix, entry) in COMPONENTS
                .iter()
                .enumerate()
                .filter(|(_, c)| c.category == *category)
            {
                let selected = ix == self.selected;
                list = list.child(
                    div()
                        .id(SharedString::from(entry.slug))
                        .h(px(30.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .rounded(px(8.))
                        .text_size(px(13.5))
                        .text_color(if selected {
                            chrome.text
                        } else {
                            chrome.text.opacity(0.78)
                        })
                        .when(selected, |d| d.font_weight(FontWeight::MEDIUM))
                        .hover_fill(
                            format!("gallery-nav-{}", entry.slug),
                            if selected {
                                chrome.selected
                            } else {
                                chrome.hover.opacity(0.)
                            },
                            if selected {
                                chrome.selected
                            } else {
                                chrome.hover
                            },
                        )
                        .cursor_pointer()
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.select(ix, window, cx)),
                        )
                        .child(entry.name),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .w(px(232.))
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(chrome.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .px(px(18.))
                    .pt(px(20.))
                    .pb(px(10.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(chrome.brand)
                            .child("pink-ui"),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(chrome.muted)
                            .child(format!("{} components in Rust and GPUI", COMPONENTS.len())),
                    ),
            )
            .child(list)
    }

    fn header(&self, chrome: &Chrome, cx: &mut Context<Self>) -> impl IntoElement {
        let entry = &COMPONENTS[self.selected];
        let is_dark = cx.is_dark();
        let pill = |id: &'static str, label: &'static str, active: bool| {
            div()
                .id(id)
                .h(px(28.))
                .px(px(12.))
                .flex()
                .items_center()
                .rounded(px(8.))
                .text_size(px(13.))
                .cursor_pointer()
                .text_color(if active { chrome.text } else { chrome.muted })
                .when(active, |d| d.bg(chrome.selected))
                .hover(|d| d.text_color(chrome.text))
                .child(label)
        };
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(24.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(chrome.text)
                                    .child(entry.name),
                            )
                            .child(
                                div()
                                    .px(px(8.))
                                    .h(px(22.))
                                    .flex()
                                    .items_center()
                                    .rounded(px(999.))
                                    .bg(chrome.hover)
                                    .text_size(px(12.))
                                    .text_color(chrome.muted)
                                    .child(entry.category),
                            ),
                    )
                    .child(
                        div()
                            .max_w(px(560.))
                            .text_size(px(14.))
                            .text_color(chrome.muted)
                            .child(entry.description),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(4.))
                    .p(px(3.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(chrome.border)
                    .child(pill("replay", "Replay", false).on_click(
                        cx.listener(|this, _, window, cx| this.replay(&Replay, window, cx)),
                    ))
                    .child(
                        pill("light", "Light", !is_dark).on_click(
                            cx.listener(|_, _, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
                        ),
                    )
                    .child(
                        pill("dark", "Dark", is_dark).on_click(
                            cx.listener(|_, _, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
                        ),
                    ),
            )
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        pink_ui::Theme::sync_rem_size(window, cx);
        pink_ui::motion::frame(window, cx);
        let chrome = Chrome::get(cx);
        let root = div()
            .id("gallery")
            .key_context("Gallery")
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::toggle_theme))
            .on_action(cx.listener(Self::replay))
            .size_full()
            .flex()
            .font_family(SANS)
            .text_color(chrome.text)
            .bg(chrome.bg);

        if self.stage_only {
            return root.child(self.stage(&chrome));
        }

        root.child(self.sidebar(&chrome, cx)).child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(20.))
                .p(px(28.))
                .child(self.header(&chrome, cx))
                .child(self.stage(&chrome)),
        )
    }
}

fn main() {
    let args = parse_args();
    let selected = args
        .slug
        .as_deref()
        .and_then(|slug| COMPONENTS.iter().position(|c| c.slug == slug))
        .unwrap_or(0);
    if let Some(slug) = args.slug.as_deref()
        && COMPONENTS[selected].slug != slug
    {
        eprintln!("gallery: unknown component `{slug}`, try --list");
    }

    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            pink_ui::init(cx);
            Theme::set_mode(args.mode, cx);
            if let Some(size) = args.ui_size {
                Theme::set_ui_font_size(size, cx);
            }
            cx.bind_keys([
                KeyBinding::new("ctrl-down", Next, Some("Gallery")),
                KeyBinding::new("ctrl-up", Previous, Some("Gallery")),
                KeyBinding::new("ctrl-t", ToggleTheme, Some("Gallery")),
                KeyBinding::new("ctrl-r", Replay, Some("Gallery")),
                KeyBinding::new("secondary-q", Quit, None),
            ]);
            cx.on_action(|_: &Quit, cx| cx.quit());

            let (w, h) = args.size.unwrap_or(if args.stage_only {
                (894., 520.)
            } else {
                (1280., 860.)
            });
            let bounds = if args.stage_only {
                Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(w), px(h)),
                }
            } else {
                Bounds::centered(None, size(px(w), px(h)), cx)
            };
            let stage_only = args.stage_only;
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(format!("{} in pink-ui", COMPONENTS[selected].name).into()),
                        ..Default::default()
                    }),
                    app_id: Some("pink-ui-gallery".into()),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| Gallery::new(selected, stage_only, window, cx)),
            )
            .expect("failed to open the gallery window");
            cx.activate(true);
        });
}
