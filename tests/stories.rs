//! Renders every component's story in both themes on GPUI's test
//! platform, so a panic in any layout, state or paint path fails the build.

use gpui::{AnyView, Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{Theme, ThemeMode, components::COMPONENTS};

/// The window root: shows whichever story the test swaps in. Drawing through
/// a real window (rather than a one-off `draw`) also paints deferred overlays.
struct Host {
    story: Option<AnyView>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.story.clone())
    }
}

#[gpui::test]
fn every_story_renders(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let (host, cx) = cx.add_window_view(|_, _| Host { story: None });
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        cx.update(|_, cx| Theme::set_mode(mode, cx));
        for entry in COMPONENTS {
            cx.update(|window, cx| {
                let story = (entry.story)(window, cx);
                host.update(cx, |host, cx| {
                    host.story = Some(story);
                    cx.notify();
                });
            });
            // Twice: the first frame mounts state, the second renders with it.
            for _ in 0..2 {
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
            }
        }
    }
}

#[test]
fn slugs_are_unique_and_described() {
    let mut slugs: Vec<_> = COMPONENTS.iter().map(|c| c.slug).collect();
    slugs.sort();
    slugs.dedup();
    assert_eq!(slugs.len(), COMPONENTS.len());
    for c in COMPONENTS {
        assert!(!c.name.is_empty(), "{} has no name", c.slug);
        assert!(!c.description.is_empty(), "{} has no description", c.slug);
    }
}
