//! Wraps a component builder into a view for the gallery.
//!
//! Components are plain builders (`ProfileMenu::new(..)`), so a story is
//! just a closure that builds one. Every story gets its own view entity, and
//! element state is namespaced by view, so recreating the story (the
//! gallery's Replay) starts the component fresh.

use gpui::{AnyElement, AnyView, App, Context, IntoElement, Render, Window, div, prelude::*};

use crate::{motion, primitives::SANS};

type Builder = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

pub struct Story {
    build: Builder,
}

impl Render for Story {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let element = (self.build)(window, cx);
        motion::keep_alive(window);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .font_family(SANS)
            .child(element)
    }
}

/// A view that renders `build` every frame.
pub fn view<E: IntoElement>(
    cx: &mut App,
    build: impl Fn(&mut Window, &mut App) -> E + 'static,
) -> AnyView {
    cx.new(|_| Story {
        build: Box::new(move |window, cx| build(window, cx).into_any_element()),
    })
    .into()
}
