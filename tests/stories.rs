//! Renders every component's story in each theme and look on GPUI's test
//! platform, so a panic in any layout, state or paint path fails the build.

use affectui::{Look, Theme, ThemeMode, components::COMPONENTS};
use gpui::{TestAppContext, point, px, size};

#[gpui::test]
fn every_story_renders(cx: &mut TestAppContext) {
    cx.update(affectui::init);
    let cx = cx.add_empty_window();
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for look in [Look::Clean, Look::Classic] {
            cx.update(|_, cx| {
                Theme::set_mode(mode, cx);
                Theme::set_look(look, cx);
            });
            for entry in COMPONENTS {
                // Twice: the first frame mounts state, the second renders with it.
                for _ in 0..2 {
                    cx.draw(point(px(0.), px(0.)), size(px(894.), px(800.)), |window, cx| {
                        (entry.story)(window, cx)
                    });
                    cx.run_until_parked();
                }
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
