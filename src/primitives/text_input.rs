//! A text field for GPUI – single line or a wrapping multi-line area.
//!
//! GPUI ships no text input, so this one is built on the platform input
//! handler (IME, dead keys and the character palette work). It is adapted
//! from GPUI's `examples/input.rs` (Apache-2.0) and grows it with multi-line
//! wrapping, vertical movement, horizontal scrolling, placeholder and caret
//! colors, and events the components listen to.
//!
//! Render it inside a styled parent: font family, size, line height and text
//! color are inherited from there, just like a native `<input>` would.

use std::{ops::Range, rc::Rc};

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    SharedString, Style, TextAlign, TextRun, TextStyle, UTF16Selection, UnderlineStyle, Window,
    WrappedLine, actions, div, fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    affect_text_input,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        Left,
        Right,
        Up,
        Down,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        Enter,
        Newline,
        Escape,
        Tab,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
    ]
);

const KEY_CONTEXT: &str = "AffectTextInput";

/// Registers the key bindings. Called once from [`crate::init`].
pub fn bind_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("alt-left", WordLeft, ctx),
        KeyBinding::new("alt-right", WordRight, ctx),
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("secondary-a", SelectAll, ctx),
        KeyBinding::new("secondary-v", Paste, ctx),
        KeyBinding::new("secondary-c", Copy, ctx),
        KeyBinding::new("secondary-x", Cut, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Enter, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("escape", Escape, ctx),
        KeyBinding::new("tab", Tab, ctx),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
    ]);
}

/// What a [`TextInput`] tells its owner. Subscribe with `cx.subscribe`.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    /// The text changed (typing, paste, cut, programmatic `set_text` excluded).
    Change(SharedString),
    /// Enter was pressed. In a multi-line field this only fires when
    /// `submit_on_enter` is set; Shift+Enter always inserts a newline there.
    Submit(SharedString),
    /// Arrow up/down in a single-line field, for moving through a list below it.
    MoveUp,
    MoveDown,
    Escape,
    Tab,
    Focus,
    Blur,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    multiline: bool,
    submit_on_enter: bool,
    max_len: Option<usize>,
    disabled: bool,
    placeholder_color: Option<Hsla>,
    caret_color: Option<Hsla>,
    selection_color: Option<Hsla>,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    is_selecting: bool,
    scroll_x: Pixels,
    layout: Option<InputLayout>,
}

/// The last painted layout, used to map mouse positions and vertical moves.
#[derive(Clone)]
struct InputLayout {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    /// Hard lines with their starting byte offset and top edge.
    lines: Rc<[(usize, Pixels, WrappedLine)]>,
}

impl EventEmitter<InputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        cx.on_focus(&focus_handle, window, |_, _, cx| cx.emit(InputEvent::Focus))
            .detach();
        cx.on_blur(&focus_handle, window, |this: &mut Self, _, cx| {
            this.is_selecting = false;
            cx.emit(InputEvent::Blur)
        })
        .detach();
        Self {
            focus_handle,
            content: SharedString::default(),
            placeholder: SharedString::default(),
            multiline: false,
            submit_on_enter: false,
            max_len: None,
            disabled: false,
            placeholder_color: None,
            caret_color: None,
            selection_color: None,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            is_selecting: false,
            scroll_x: px(0.),
            layout: None,
        }
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Wraps text and lets Shift+Enter (or Enter, see `submit_on_enter`) add lines.
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }

    /// In a multi-line field, Enter submits and Shift+Enter adds a line.
    pub fn submit_on_enter(mut self) -> Self {
        self.submit_on_enter = true;
        self
    }

    /// Caps the length in characters (e.g. one digit per OTP slot).
    pub fn max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    pub fn default_text(mut self, text: impl Into<SharedString>) -> Self {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self
    }

    pub fn placeholder_color(mut self, color: impl Into<Hsla>) -> Self {
        self.placeholder_color = Some(color.into());
        self
    }

    pub fn caret_color(mut self, color: impl Into<Hsla>) -> Self {
        self.caret_color = Some(color.into());
        self
    }

    pub fn selection_color(mut self, color: impl Into<Hsla>) -> Self {
        self.selection_color = Some(color.into());
        self
    }

    pub fn text(&self) -> SharedString {
        self.content.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Replaces the text without emitting [`InputEvent::Change`]; the caret moves to the end.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text("", cx);
        self.scroll_x = px(0.);
    }

    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.placeholder = text.into();
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        cx.notify();
    }

    pub fn set_colors(
        &mut self,
        placeholder: Option<Hsla>,
        caret: Option<Hsla>,
        selection: Option<Hsla>,
    ) {
        self.placeholder_color = placeholder;
        self.caret_color = caret;
        self.selection_color = selection;
    }

    /// Focuses `input`; an associated function because focusing needs the app
    /// mutably while `input.read(cx)` would hold it.
    pub fn focus(input: &Entity<Self>, window: &mut Window, cx: &mut App) {
        let handle = input.read(cx).focus_handle.clone();
        window.focus(&handle, cx);
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify()
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word(self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word(self.cursor_offset()), cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            cx.emit(InputEvent::MoveUp);
            return;
        }
        let target = self.vertical_target(self.cursor_offset(), -1.);
        self.move_to(target, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            cx.emit(InputEvent::MoveDown);
            return;
        }
        let target = self.vertical_target(self.cursor_offset(), 1.);
        self.move_to(target, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical_target(self.cursor_offset(), -1.);
        self.select_to(target, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.vertical_target(self.cursor_offset(), 1.);
        self.select_to(target, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.cursor_offset();
        let start = self.content[..offset].rfind('\n').map_or(0, |i| i + 1);
        self.move_to(start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.cursor_offset();
        let end = self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |i| offset + i);
        self.move_to(end, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_word(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline && !self.submit_on_enter {
            self.replace_text_in_range(None, "\n", window, cx);
        } else {
            cx.emit(InputEvent::Submit(self.content.clone()));
        }
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.replace_text_in_range(None, "\n", window, cx);
        } else {
            cx.emit(InputEvent::Submit(self.content.clone()));
        }
    }

    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Escape);
    }

    fn tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Tab);
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = if self.multiline {
                text
            } else {
                text.replace('\n', " ")
            };
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        self.is_selecting = true;
        window.focus(&self.focus_handle, cx);
        let index = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(index, cx);
        } else if event.click_count == 2 {
            let start = self.previous_word(self.next_boundary(index));
            let end = self.next_word(index);
            self.move_to(start, cx);
            self.select_to(end, cx);
        } else if event.click_count >= 3 {
            self.move_to(0, cx);
            self.select_to(self.content.len(), cx);
        } else {
            self.move_to(index, cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = self.layout.as_ref() else {
            return 0;
        };
        if self.content.is_empty() {
            return 0;
        }
        let local = point(
            position.x - layout.bounds.left() + self.scroll_x,
            position.y - layout.bounds.top(),
        );
        self.index_for_local_point(layout, local)
    }

    fn index_for_local_point(&self, layout: &InputLayout, local: Point<Pixels>) -> usize {
        if local.y < px(0.) {
            return 0;
        }
        let mut target = None;
        for (start, top, line) in layout.lines.iter() {
            let height = layout.line_height * (line.wrap_boundaries().len() + 1) as f32;
            if local.y < *top + height {
                target = Some((*start, *top, line));
                break;
            }
        }
        let Some((start, top, line)) = target else {
            return self.content.len();
        };
        let in_line = point(local.x.max(px(0.)), local.y - top);
        let index = match line.closest_index_for_position(in_line, layout.line_height) {
            Ok(i) | Err(i) => i,
        };
        (start + index).min(self.content.len())
    }

    fn caret_point(&self, layout: &InputLayout, offset: usize) -> Option<Point<Pixels>> {
        for (start, top, line) in layout.lines.iter() {
            if offset >= *start && offset <= start + line.len() {
                let p = line.position_for_index(offset - start, layout.line_height)?;
                return Some(point(p.x, p.y + *top));
            }
        }
        None
    }

    fn vertical_target(&self, offset: usize, direction: f32) -> usize {
        let Some(layout) = self.layout.as_ref() else {
            return offset;
        };
        let Some(caret) = self.caret_point(layout, offset) else {
            return offset;
        };
        let y = caret.y + layout.line_height * (0.5 + direction);
        if y < px(0.) {
            return 0;
        }
        self.index_for_local_point(layout, point(caret.x, y))
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    fn previous_word(&self, offset: usize) -> usize {
        self.content
            .split_word_bound_indices()
            .rev()
            .find_map(|(idx, word)| (idx < offset && !word.trim().is_empty()).then_some(idx))
            .unwrap_or(0)
    }

    fn next_word(&self, offset: usize) -> usize {
        self.content
            .split_word_bound_indices()
            .find_map(|(idx, word)| {
                let end = idx + word.len();
                (end > offset && !word.trim().is_empty()).then_some(end)
            })
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        let mut new_text = if self.multiline {
            new_text.to_string()
        } else {
            new_text.replace('\n', " ")
        };
        if let Some(max) = self.max_len {
            let kept = self.content.chars().count() - self.content[range.clone()].chars().count();
            let room = max.saturating_sub(kept);
            new_text = new_text.chars().take(room).collect();
        }

        self.content =
            (self.content[0..range.start].to_owned() + &new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
        cx.emit(InputEvent::Change(self.content.clone()));
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.emit(InputEvent::Change(self.content.clone()));
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = self.caret_point(layout, range.start)?;
        let end = self.caret_point(layout, range.end)?;
        Some(Bounds::from_corners(
            point(
                bounds.left() + start.x - self.scroll_x,
                bounds.top() + start.y,
            ),
            point(
                bounds.left() + end.x - self.scroll_x,
                bounds.top() + end.y + layout.line_height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let index = self.index_for_mouse_position(point);
        Some(self.offset_to_utf16(index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    lines: Rc<[(usize, Pixels, WrappedLine)]>,
    line_height: Pixels,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
    scroll_x: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// The inherited text style, captured where it is valid: layout measurement
/// runs outside the element's style stack, where `window.text_style()` would
/// return the root style.
#[derive(Clone)]
struct InheritedText {
    style: TextStyle,
    line_height: Pixels,
    font_size: Pixels,
}

impl InheritedText {
    fn capture(window: &Window) -> Self {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        Self {
            style,
            line_height: window.line_height(),
            font_size,
        }
    }
}

/// Shapes the visible text into hard lines, wrapped to `wrap_width` when given.
fn shape(
    input: &TextInput,
    text: &InheritedText,
    window: &mut Window,
    wrap_width: Option<Pixels>,
) -> (Rc<[(usize, Pixels, WrappedLine)]>, Pixels) {
    let style = &text.style;
    let line_height = text.line_height;
    let font_size = text.font_size;

    let (display_text, color) = if input.content.is_empty() {
        let placeholder = input
            .placeholder_color
            .unwrap_or_else(|| style.color.opacity(0.45));
        (input.placeholder.clone(), placeholder)
    } else {
        (input.content.clone(), style.color)
    };

    let run = TextRun {
        len: display_text.len(),
        font: style.font(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let runs: Vec<TextRun> = match input.marked_range.as_ref() {
        Some(marked) if !input.content.is_empty() => vec![
            TextRun {
                len: marked.start,
                ..run.clone()
            },
            TextRun {
                len: marked.end - marked.start,
                underline: Some(UnderlineStyle {
                    color: Some(run.color),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..run.clone()
            },
            TextRun {
                len: display_text.len() - marked.end,
                ..run
            },
        ]
        .into_iter()
        .filter(|run| run.len > 0)
        .collect(),
        _ => vec![run],
    };

    let wrapped = window
        .text_system()
        .shape_text(display_text, font_size, &runs, wrap_width, None)
        .unwrap_or_default();

    let mut lines = Vec::with_capacity(wrapped.len());
    let mut start = 0;
    let mut top = px(0.);
    for line in wrapped {
        let rows = line.wrap_boundaries().len() + 1;
        let len = line.len();
        lines.push((start, top, line));
        start += len + 1;
        top += line_height * rows as f32;
    }
    (lines.into(), line_height)
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let multiline = self.input.read(cx).multiline;
        if !multiline {
            style.size.height = window.line_height().into();
            return (window.request_layout(style, [], cx), ());
        }
        let input = self.input.clone();
        let text = InheritedText::capture(window);
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, cx| {
                let width = known.width.or(match available.width {
                    gpui::AvailableSpace::Definite(w) => Some(w),
                    _ => None,
                });
                let (lines, line_height) = shape(input.read(cx), &text, window, width);
                let rows: usize = lines
                    .iter()
                    .map(|(_, _, l)| l.wrap_boundaries().len() + 1)
                    .sum();
                size(width.unwrap_or(px(0.)), line_height * rows.max(1) as f32)
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let wrap = input.multiline.then_some(bounds.size.width);
        let text = InheritedText::capture(window);
        let (lines, line_height) = shape(input, &text, window, wrap);
        let layout = InputLayout {
            bounds,
            line_height,
            lines: lines.clone(),
        };

        let text_color = window.text_style().color;
        let caret_color = input.caret_color.unwrap_or(text_color);
        let selection_color = input
            .selection_color
            .unwrap_or_else(|| gpui::hsla(0.6, 0.9, 0.55, 0.28));

        let cursor_offset = if input.content.is_empty() {
            0
        } else {
            input.cursor_offset()
        };
        let caret = input
            .caret_point(&layout, cursor_offset)
            .unwrap_or_default();

        // Keep the caret in view when a single line overflows.
        let mut scroll_x = input.scroll_x;
        if !input.multiline {
            let width = bounds.size.width;
            let text_width = lines.first().map(|(_, _, l)| l.width()).unwrap_or_default();
            if caret.x - scroll_x > width - px(2.) {
                scroll_x = caret.x - width + px(2.);
            } else if caret.x < scroll_x {
                scroll_x = caret.x;
            }
            scroll_x = scroll_x
                .min((text_width - width + px(2.)).max(px(0.)))
                .max(px(0.));
        } else {
            scroll_x = px(0.);
        }

        let origin = point(bounds.left() - scroll_x, bounds.top());
        let mut selections = Vec::new();
        let range = input.selected_range.clone();
        if !range.is_empty() && !input.content.is_empty() {
            for (start, top, line) in lines.iter() {
                let line_end = start + line.len();
                let s = range.start.max(*start);
                let e = range.end.min(line_end);
                if s > e || (s == e && range.end <= *start) {
                    continue;
                }
                let (Some(a), Some(b)) = (
                    line.position_for_index(s - start, line_height),
                    line.position_for_index(e - start, line_height),
                ) else {
                    continue;
                };
                let row_width = |_: Pixels| bounds.size.width + scroll_x;
                let mut y = a.y;
                while y <= b.y {
                    let x0 = if y == a.y { a.x } else { px(0.) };
                    let x1 = if y == b.y {
                        // Show a sliver for selected newlines
                        if e == line_end && range.end > line_end {
                            b.x + px(4.)
                        } else {
                            b.x
                        }
                    } else {
                        row_width(y)
                    };
                    if x1 > x0 {
                        selections.push(fill(
                            Bounds::from_corners(
                                point(origin.x + x0, origin.y + *top + y),
                                point(origin.x + x1, origin.y + *top + y + line_height),
                            ),
                            selection_color,
                        ));
                    }
                    y += line_height;
                }
            }
        }

        let cursor = (input.selected_range.is_empty() && !input.disabled).then(|| {
            fill(
                Bounds::new(
                    point(origin.x + caret.x, origin.y + caret.y + line_height * 0.12),
                    size(px(1.5), line_height * 0.76),
                ),
                caret_color,
            )
        });

        self.input.update(cx, |input, _| input.scroll_x = scroll_x);
        PrepaintState {
            lines,
            line_height,
            cursor,
            selections,
            scroll_x,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let line_height = prepaint.line_height;
        let origin = point(bounds.left() - prepaint.scroll_x, bounds.top());
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for selection in prepaint.selections.drain(..) {
                window.paint_quad(selection);
            }
            for (_, top, line) in prepaint.lines.iter() {
                line.paint(
                    point(origin.x, origin.y + *top),
                    line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
        });

        let lines = prepaint.lines.clone();
        self.input.update(cx, |input, _| {
            input.layout = Some(InputLayout {
                bounds,
                line_height,
                lines,
            });
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("affect-text-input")
            .flex()
            .w_full()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle(cx))
            .when(!self.disabled, |this| this.cursor(CursorStyle::IBeam))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::tab))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}
