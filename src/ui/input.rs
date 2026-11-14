//! Minimal but complete text widgets built directly on GPUI's element API.
//!
//! - [`TextInput`]: single line, used for search and form fields.
//! - [`TextArea`]: wrapping multi-line editor used for writing mail.

use std::ops::Range;

use gpui::{
    actions, div, fill, point, prelude::*, px, size, App, Bounds, ClipboardItem, Context,
    CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    feather_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Up,
        Down,
    ]
);

#[derive(Clone, Copy)]
pub struct TextColors {
    pub caret: Hsla,
    pub selection: Hsla,
    pub placeholder: Hsla,
}

impl Default for TextColors {
    fn default() -> Self {
        Self {
            caret: gpui::rgb(0x8296FF).into(),
            selection: gpui::rgba(0x8296FF3D).into(),
            placeholder: gpui::rgb(0x79839A).into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Single line input
// ---------------------------------------------------------------------------

pub struct TextInput {
    content: SharedString,
    placeholder: SharedString,
    focus_handle: FocusHandle,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    colors: TextColors,
    font_size: Pixels,
    line_height: Pixels,
}

impl TextInput {
    pub fn new(placeholder: &str, value: &str, cx: &mut Context<Self>) -> Self {
        Self {
            content: value.to_string().into(),
            placeholder: placeholder.to_string().into(),
            focus_handle: cx.focus_handle(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            colors: TextColors::default(),
            font_size: px(13.5),
            line_height: px(20.),
        }
    }

    pub fn value(&self) -> &str {
        &self.content
    }

    pub fn set_value(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = value.into();
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_value("", cx);
    }

    pub fn set_colors(&mut self, colors: TextColors) {
        self.colors = colors;
    }


    pub fn handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
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

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
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

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace(['\n', '\r'], " "), window, cx);
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
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);

        self.content = (self.content[0..start].to_owned() + new_text + &self.content[end..]).into();
        self.selected_range = start + new_text.len()..start + new_text.len();
        self.marked_range.take();
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
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);

        self.content = (self.content[0..start].to_owned() + new_text + &self.content[end..]).into();
        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            self.marked_range = Some(start..start + new_text.len());
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .map(|new_range| new_range.start + start..new_range.end + start)
            .unwrap_or_else(|| start + new_text.len()..start + new_text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextInputElement {
    input: Entity<TextInput>,
}

struct TextInputPrepaint {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = TextInputPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line_height = self.input.read(cx).line_height;
        let mut style = Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = line_height.into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();
        let colors = input.colors;
        let font_size = input.font_size;

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), colors.placeholder)
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len().saturating_sub(marked_range.end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = if font_size == px(0.) {
            style.font_size.to_pixels(window.rem_size())
        } else {
            font_size
        };
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let line_height = input.line_height;
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top() + px(1.)),
                        size(px(1.5), line_height - px(2.)),
                    ),
                    colors.caret,
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top() + px(1.),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom() - px(1.),
                        ),
                    ),
                    colors.selection,
                )),
                None,
            )
        };

        TextInputPrepaint {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
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
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        let line_height = self.input.read(cx).line_height;
        line.paint(bounds.origin, line_height, window, cx).ok();
        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }
        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .w_full()
            .key_context("TextInput")
            .track_focus(&self.handle())
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextInputElement { input: cx.entity() })
    }
}

// ---------------------------------------------------------------------------
// Multi line text area
// ---------------------------------------------------------------------------

struct AreaLayout {
    lines: Vec<Option<WrappedLine>>,
    starts: Vec<usize>,
    tops: Vec<Pixels>,
    total_height: Pixels,
}

pub struct TextArea {
    content: SharedString,
    placeholder: SharedString,
    focus_handle: FocusHandle,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    width: Pixels,
    font_size: Pixels,
    line_height: Pixels,
    colors: TextColors,
    bounds: Option<Bounds<Pixels>>,
    layout: Option<AreaLayout>,
    is_selecting: bool,
}

impl TextArea {
    pub fn new(
        placeholder: &str,
        value: &str,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            content: value.to_string().into(),
            placeholder: placeholder.to_string().into(),
            focus_handle: cx.focus_handle(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            width,
            font_size: px(14.),
            line_height: px(23.),
            colors: TextColors::default(),
            bounds: None,
            layout: None,
            is_selecting: false,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    #[allow(dead_code)]
    pub fn set_content(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = value.into();
        let end = self.content.len();
        self.selected_range = end..end;
        cx.notify();
    }

    pub fn set_colors(&mut self, colors: TextColors) {
        self.colors = colors;
    }

    pub fn handle(&self) -> FocusHandle {
        self.focus_handle.clone()
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
        cx.notify();
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
        cx.notify();
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

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn paragraph_of(&self, offset: usize) -> usize {
        let Some(layout) = self.layout.as_ref() else {
            return 0;
        };
        paragraph_of_starts(&layout.starts, offset)
    }

    fn caret_point(&self, offset: usize) -> Option<Point<Pixels>> {
        let layout = self.layout.as_ref()?;
        caret_point_in(layout, offset, self.line_height)
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = self.layout.as_ref() else {
            return 0;
        };
        if layout.lines.is_empty() {
            return 0;
        }
        let mut paragraph = 0;
        for (index, top) in layout.tops.iter().enumerate() {
            let height = layout
                .lines
                .get(index)
                .and_then(|line| line.as_ref())
                .map(|line| self.line_height * (line.wrap_boundaries().len() as f32 + 1.))
                .unwrap_or(self.line_height);
            if position.y >= *top && position.y < *top + height {
                paragraph = index;
                break;
            }
            if position.y >= *top + height {
                paragraph = index;
            }
        }
        let start = layout.starts.get(paragraph).copied().unwrap_or(0);
        let top = layout.tops.get(paragraph).copied().unwrap_or(px(0.));
        let Some(line) = layout.lines.get(paragraph).and_then(|line| line.as_ref()) else {
            return start;
        };
        let local_y = (position.y - top).max(px(0.));
        let local = line
            .closest_index_for_position(point(position.x, local_y), self.line_height)
            .unwrap_or_else(|index| index);
        start + local.min(line.len())
    }


    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let cursor = self.cursor_offset();
            if cursor == 0 {
                return;
            }
            self.select_to(self.previous_boundary(cursor), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let cursor = self.cursor_offset();
            if cursor >= self.content.len() {
                return;
            }
            self.select_to(self.next_boundary(cursor), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
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
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let paragraph = self.paragraph_of(cursor);
        let start = self
            .layout
            .as_ref()
            .and_then(|layout| layout.starts.get(paragraph).copied())
            .unwrap_or(0);
        self.move_to(start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let paragraph = self.paragraph_of(cursor);
        let end = self
            .layout
            .as_ref()
            .and_then(|layout| {
                let next = layout.starts.get(paragraph + 1).copied();
                Some(match next {
                    Some(next) => next.saturating_sub(1),
                    None => self.content.len(),
                })
            })
            .unwrap_or(self.content.len());
        self.move_to(end, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        let Some(caret) = self.caret_point(self.cursor_offset()) else {
            return;
        };
        let target = point(caret.x, caret.y - self.line_height);
        let index = self.index_for_position_padded(target);
        self.move_to(index, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let Some(caret) = self.caret_point(self.cursor_offset()) else {
            return;
        };
        let target = point(caret.x, caret.y + self.line_height);
        let index = self.index_for_position_padded(target);
        self.move_to(index, cx);
    }

    fn index_for_position_padded(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = self.layout.as_ref() else {
            return 0;
        };
        let max_y = layout.total_height - px(1.);
        self.index_for_position(point(
            position.x,
            position.y.clamp(px(0.), max_y.max(px(0.))),
        ))
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
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
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        let Some(bounds) = self.bounds else { return };
        let local = point(event.position.x - bounds.left(), event.position.y - bounds.top());
        let index = self.index_for_position(local);
        if event.modifiers.shift {
            self.select_to(index, cx);
        } else {
            self.move_to(index, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.is_selecting {
            return;
        }
        let Some(bounds) = self.bounds else { return };
        let local = point(event.position.x - bounds.left(), event.position.y - bounds.top());
        let index = self.index_for_position(local);
        self.select_to(index, cx);
    }
}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TextArea {
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);
        let new_text = new_text.replace("\r\n", "\n").replace('\r', "\n");

        self.content = (self.content[0..start].to_owned() + &new_text + &self.content[end..]).into();
        self.selected_range = start + new_text.len()..start + new_text.len();
        self.selection_reversed = false;
        self.marked_range.take();
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
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len()).max(start);
        self.content = (self.content[0..start].to_owned() + new_text + &self.content[end..]).into();
        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            self.marked_range = Some(start..start + new_text.len());
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .map(|new_range| new_range.start + start..new_range.end + start)
            .unwrap_or_else(|| start + new_text.len()..start + new_text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start = self.caret_point(range.start)?;
        Some(Bounds::from_corners(
            point(bounds.left() + start.x, bounds.top() + start.y),
            point(
                bounds.left() + start.x + px(2.),
                bounds.top() + start.y + self.line_height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.bounds?;
        let local = point(position.x - bounds.left(), position.y - bounds.top());
        let index = self.index_for_position(local);
        Some(self.offset_to_utf16(index))
    }
}

struct TextAreaElement {
    input: Entity<TextArea>,
}

struct TextAreaPrepaint {
    layout: AreaLayout,
    selection: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

impl IntoElement for TextAreaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextAreaElement {
    type RequestLayoutState = AreaLayout;
    type PrepaintState = TextAreaPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let width = input.width;
        let font_size = input.font_size;
        let line_height = input.line_height;
        let style = window.text_style().clone();

        let layout = shape_area(&mut *window, &style, &content, width, font_size, line_height);

        let mut style = Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = layout.total_height.into();
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, layout)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let line_height = input.line_height;
        let colors = input.colors;
        let selected = input.selected_range.clone();
        let cursor_index = input.cursor_offset();
        let width = input.width;
        let layout = std::mem::replace(request_layout, empty_area_layout());

        let mut selection = Vec::new();
        if !selected.is_empty() {
            for (index, line) in layout.lines.iter().enumerate() {
                let Some(line) = line else { continue };
                let top = layout.tops[index];
                let start = layout.starts[index];
                let end = start + line.len();
                if selected.end <= start || selected.start > end {
                    continue;
                }
                let local_start = selected.start.saturating_sub(start).min(line.len());
                let local_end = selected.end.saturating_sub(start).min(line.len());
                if local_start == local_end && !(selected.start <= start && selected.end > end) {
                    continue;
                }
                let p1 = line
                    .position_for_index(local_start, line_height)
                    .unwrap_or_else(|| point(px(0.), px(0.)));
                let p2 = line
                    .position_for_index(local_end, line_height)
                    .unwrap_or_else(|| point(width, px(0.)));
                let mut y = top + p1.y;
                if p1.y == p2.y {
                    selection.push(fill(
                        Bounds::new(
                            point(bounds.left() + p1.x, bounds.top() + y + px(3.)),
                            size((p2.x - p1.x).max(px(1.)), line_height - px(6.)),
                        ),
                        colors.selection,
                    ));
                } else {
                    selection.push(fill(
                        Bounds::new(
                            point(bounds.left() + p1.x, bounds.top() + y + px(3.)),
                            size((width - p1.x).max(px(1.)), line_height - px(6.)),
                        ),
                        colors.selection,
                    ));
                    y += line_height;
                    while y < top + p2.y {
                        selection.push(fill(
                            Bounds::new(
                                point(bounds.left(), bounds.top() + y + px(3.)),
                                size(width, line_height - px(6.)),
                            ),
                            colors.selection,
                        ));
                        y += line_height;
                    }
                    selection.push(fill(
                        Bounds::new(
                            point(bounds.left(), bounds.top() + y + px(3.)),
                            size(p2.x.max(px(2.)), line_height - px(6.)),
                        ),
                        colors.selection,
                    ));
                }
            }
        }

        let cursor = caret_point_in(&layout, cursor_index, line_height).map(|offset| {
            fill(
                Bounds::new(
                    point(bounds.left() + offset.x, bounds.top() + offset.y + px(3.)),
                    size(px(1.7), line_height - px(6.)),
                ),
                colors.caret,
            )
        });

        TextAreaPrepaint {
            layout,
            selection,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
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

        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }

        let line_height = self.input.read(cx).line_height;
        let colored: Vec<Option<WrappedLine>> = prepaint.layout.lines.drain(..).collect();
        for (index, line) in colored.iter().enumerate() {
            if let Some(line) = line {
                let top = prepaint.layout.tops[index];
                line.paint(
                    point(bounds.left(), bounds.top() + top),
                    line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
            }
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        let layout = AreaLayout {
            lines: colored,
            starts: prepaint.layout.starts.clone(),
            tops: prepaint.layout.tops.clone(),
            total_height: prepaint.layout.total_height,
        };
        self.input.update(cx, |input, _cx| {
            input.layout = Some(layout);
            input.bounds = Some(bounds);
        });
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let line_height = self.line_height;
        let placeholder = self.placeholder.clone();
        let is_empty = self.content.is_empty();
        let placeholder_color = colors.placeholder;
        let style = window.text_style();

        div()
            .flex()
            .flex_col()
            .relative()
            .w_full()
            .key_context("TextArea")
            .track_focus(&self.handle())
            .cursor(CursorStyle::IBeam)
            .text_size(self.font_size)
            .line_height(line_height)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(is_empty, |this| {
                this.child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .text_size(style.font_size)
                        .text_color(placeholder_color)
                        .child(placeholder),
                )
            })
            .child(TextAreaElement { input: cx.entity() })
    }
}

fn text_runs(text: &str, style: &gpui::TextStyle, _font_size: Pixels) -> Vec<TextRun> {
    if text.is_empty() {
        return Vec::new();
    }
    vec![TextRun {
        len: text.len(),
        font: style.font(),
        color: style.color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }]
}

fn empty_area_layout() -> AreaLayout {
    AreaLayout {
        lines: Vec::new(),
        starts: Vec::new(),
        tops: Vec::new(),
        total_height: px(0.),
    }
}

fn shape_area(
    window: &mut Window,
    style: &gpui::TextStyle,
    content: &str,
    width: Pixels,
    font_size: Pixels,
    line_height: Pixels,
) -> AreaLayout {
    let mut lines = Vec::new();
    let mut starts = Vec::new();
    let mut tops = Vec::new();
    let mut y = px(0.);
    let mut offset = 0;

    for paragraph in content.split('\n') {
        starts.push(offset);
        let line = if paragraph.is_empty() {
            None
        } else {
            let runs = text_runs(paragraph, style, font_size);
            window
                .text_system()
                .shape_text(
                    SharedString::from(paragraph.to_string()),
                    font_size,
                    &runs,
                    Some(width),
                    None,
                )
                .ok()
                .and_then(|mut shaped| shaped.pop())
        };
        let height = line
            .as_ref()
            .map(|line| line_height * (line.wrap_boundaries().len() as f32 + 1.))
            .unwrap_or(line_height);
        tops.push(y);
        y += height;
        lines.push(line);
        offset += paragraph.len() + 1;
    }

    AreaLayout {
        lines,
        starts,
        tops,
        total_height: y.max(line_height),
    }
}

fn paragraph_of_starts(starts: &[usize], offset: usize) -> usize {
    if starts.is_empty() {
        return 0;
    }
    match starts.binary_search(&offset) {
        Ok(index) => index,
        Err(index) => index.saturating_sub(1),
    }
}

fn caret_point_in(layout: &AreaLayout, offset: usize, line_height: Pixels) -> Option<Point<Pixels>> {
    let paragraph = paragraph_of_starts(&layout.starts, offset);
    let start = *layout.starts.get(paragraph)?;
    let top = *layout.tops.get(paragraph)?;
    let local = offset.saturating_sub(start);
    let Some(line) = layout.lines.get(paragraph).and_then(|line| line.as_ref()) else {
        // Empty paragraph: still show the caret at the start of the line.
        return Some(point(Pixels::ZERO, top));
    };
    let position = line.position_for_index(local.min(line.len()), line_height)?;
    Some(point(position.x, top + position.y))
}




