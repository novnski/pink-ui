//! Wraps a component builder into a view for the gallery.
//!
//! Components are plain builders (`ProfileMenu::new(..)`), so a story is
//! just a closure that builds one. Every story gets its own view entity, and
//! element state is namespaced by view, so recreating the story (the
//! gallery's Replay) starts the component fresh.
//!
//! The view is the demo's host, the way Pink's shell is: it holds focus when
//! nothing else does and moves it on Tab and Shift Tab (Pink `shell.rs`
//! routes Tab to `focus_next` / `focus_prev` at its root), so every tab stop
//! a component declares is reachable from the keyboard. Components that
//! handle Tab themselves stop propagation first.

use std::{cell::Cell, rc::Rc};

use gpui::{
    AnyElement, AnyView, App, Context, FocusHandle, IntoElement, KeyDownEvent, Pixels, Render,
    Size, Window, div, prelude::*,
};

use crate::{motion, pink};

type Builder = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

pub struct Story {
    build: Builder,
    content: Rc<Cell<Size<Pixels>>>,
    focus: FocusHandle,
}

impl Story {
    /// The demo's size as last laid out, for hosts that fit their stage to
    /// it (the web player). Zero until the first frame.
    pub fn content_size(&self) -> Size<Pixels> {
        self.content.get()
    }
}

impl Render for Story {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let element = (self.build)(window, cx);
        let content = self.content.clone();
        if window.focused(cx).is_none() {
            window.focus(&self.focus, cx);
        }
        div()
            .track_focus(&self.focus)
            .on_key_down(|event: &KeyDownEvent, window, cx| {
                let m = event.keystroke.modifiers;
                if event.keystroke.key != "tab" || m.control || m.alt || m.platform {
                    return;
                }
                if m.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                cx.stop_propagation();
            })
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            // Every demo sits on Pink's content panel, as it would in the app.
            .bg(pink::Pink::get(cx).bg)
            .font_family(pink::SANS)
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(demo) = bounds.first() {
                    content.set(demo.size);
                }
            })
            .child(element)
            .child(motion::driver())
    }
}

/// A view that renders `build` every frame.
pub fn view<E: IntoElement>(
    cx: &mut App,
    build: impl Fn(&mut Window, &mut App) -> E + 'static,
) -> AnyView {
    cx.new(|cx| Story {
        build: Box::new(move |window, cx| build(window, cx).into_any_element()),
        content: Rc::default(),
        focus: cx.focus_handle(),
    })
    .into()
}
