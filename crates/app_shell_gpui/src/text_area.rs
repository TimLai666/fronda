//! Reusable multiline text area with soft wrap and real platform text input —
//! IME composition (CJK), cursor movement across visual lines, selection, and
//! clipboard — built on gpui's `EntityInputHandler` (same architecture as
//! [`crate::text_field`], multiline via `shape_text`/`WrappedLine`).
//!
//! Views embed an `Entity<TextArea>` and subscribe to [`TextAreaEvent`].
//! Call [`bind_text_area_keys`] once at app boot; bindings are scoped to
//! the `FrondaTextArea` key context so they never affect global shortcuts.
//! Enter inserts a newline (there is no Submit). The element grows with
//! content between `min_lines` and `max_lines`; scrolling is out of scope.

use crate::theme::{Accent, Text as ThemeText};
use gpui::{
    actions, div, fill, point, prelude::*, px, relative, size, App, AvailableSpace, Bounds,
    ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, SharedString, Style, TextRun, TextStyle, UTF16Selection, UnderlineStyle,
    Window, WrappedLine,
};
use std::ops::Range;

actions!(
    fronda_text_area,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        InsertNewline,
    ]
);

const CURSOR_WIDTH: f32 = 2.0;

/// Register the area's key bindings. Idempotent per app; call at boot.
pub fn bind_text_area_keys(cx: &mut App) {
    const CTX: Option<&str> = Some("FrondaTextArea");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, CTX),
        KeyBinding::new("delete", Delete, CTX),
        KeyBinding::new("left", Left, CTX),
        KeyBinding::new("right", Right, CTX),
        KeyBinding::new("up", Up, CTX),
        KeyBinding::new("down", Down, CTX),
        KeyBinding::new("shift-left", SelectLeft, CTX),
        KeyBinding::new("shift-right", SelectRight, CTX),
        KeyBinding::new("home", Home, CTX),
        KeyBinding::new("end", End, CTX),
        KeyBinding::new("enter", InsertNewline, CTX),
        // Both chords so one build serves macOS (cmd) and Win/Linux (ctrl).
        KeyBinding::new("cmd-a", SelectAll, CTX),
        KeyBinding::new("ctrl-a", SelectAll, CTX),
        KeyBinding::new("cmd-v", Paste, CTX),
        KeyBinding::new("ctrl-v", Paste, CTX),
        KeyBinding::new("cmd-c", Copy, CTX),
        KeyBinding::new("ctrl-c", Copy, CTX),
        KeyBinding::new("cmd-x", Cut, CTX),
        KeyBinding::new("ctrl-x", Cut, CTX),
    ]);
}

/// Events a host view can subscribe to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAreaEvent {
    /// The content changed (typing, paste, IME commit, newline).
    Edited,
}

/// Multiline editable text area entity.
pub struct TextArea {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    min_lines: usize,
    max_lines: Option<usize>,
    /// Goal column (x) preserved across consecutive up/down presses.
    preferred_x: Option<Pixels>,
    last_lines: Option<Vec<WrappedLine>>,
    last_bounds: Option<Bounds<Pixels>>,
    last_line_height: Pixels,
    is_selecting: bool,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl TextArea {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            min_lines: 1,
            max_lines: None,
            preferred_x: None,
            last_lines: None,
            last_bounds: None,
            last_line_height: px(0.),
            is_selecting: false,
        }
    }

    pub fn with_min_lines(mut self, min_lines: usize) -> Self {
        self.min_lines = min_lines.max(1);
        self
    }

    pub fn with_max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines.max(1));
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.preferred_x = None;
        cx.notify();
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    // ── Actions ──

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

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1, cx);
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
        let ranges = hard_line_ranges(&self.content);
        let line = line_containing_offset(&ranges, self.cursor_offset());
        self.move_to(ranges[line].start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let ranges = hard_line_ranges(&self.content);
        let line = line_containing_offset(&ranges, self.cursor_offset());
        self.move_to(ranges[line].end, cx);
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

    fn insert_newline(&mut self, _: &InsertNewline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            // Multiline: keep newlines, normalize Windows/mac line endings.
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
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

    // No raw key_down swallowing: the `input` marker in this element's key
    // context keeps `!input`-predicated global shortcut bindings inert while
    // the area is focused, and characters arrive via the platform text-input
    // path. Unhandled keys (escape/tab) bubble to the host view.

    // ── Mouse ──

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
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

    // ── Internals ──

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.preferred_x = None;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    /// Move the cursor one visual line up (`dir` = -1) or down (`dir` = 1),
    /// keeping the goal column across consecutive presses. Uses the wrapped
    /// geometry from the last paint; a no-op before first paint.
    fn move_vertical(&mut self, dir: i64, cx: &mut Context<Self>) {
        if self.content.is_empty() {
            return;
        }
        let Some(lines) = self.last_lines.as_ref() else {
            return;
        };
        let line_height = self.last_line_height;
        if line_height <= px(0.) {
            return;
        }
        let ranges = hard_line_ranges(&self.content);
        // Stale-layout guard: last_lines may lag the content by one frame.
        if lines.len() != ranges.len() {
            return;
        }
        let offset = self.cursor_offset();
        let line_ix = line_containing_offset(&ranges, offset);
        let local = offset - ranges[line_ix].start;
        let Some(pos) = lines[line_ix].position_for_index(local, line_height) else {
            return;
        };
        let goal_x = self.preferred_x.unwrap_or(pos.x);
        let row = (pos.y / line_height).round() as i64;
        let rows_in_line = lines[line_ix].wrap_boundaries().len() as i64 + 1;
        let target_row = row + dir;

        let new_offset = if (0..rows_in_line).contains(&target_row) {
            let y = line_height * (target_row as f32 + 0.5);
            ranges[line_ix].start + index_in_line(&lines[line_ix], point(goal_x, y), line_height)
        } else if dir < 0 {
            if line_ix == 0 {
                0
            } else {
                let prev = line_ix - 1;
                let last_row = lines[prev].wrap_boundaries().len();
                let y = line_height * (last_row as f32 + 0.5);
                ranges[prev].start + index_in_line(&lines[prev], point(goal_x, y), line_height)
            }
        } else if line_ix + 1 >= ranges.len() {
            self.content.len()
        } else {
            let next = line_ix + 1;
            let y = line_height * 0.5;
            ranges[next].start + index_in_line(&lines[next], point(goal_x, y), line_height)
        };

        let new_offset = self.clamp_to_char_boundary(new_offset);
        self.move_to(new_offset, cx);
        self.preferred_x = Some(goal_x);
    }

    fn clamp_to_char_boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.content.len());
        while offset > 0 && !self.content.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(lines)) = (self.last_bounds.as_ref(), self.last_lines.as_ref())
        else {
            return 0;
        };
        let line_height = self.last_line_height;
        if line_height <= px(0.) {
            return 0;
        }
        let ranges = hard_line_ranges(&self.content);
        if lines.len() != ranges.len() {
            return 0;
        }
        if position.y < bounds.top() {
            return 0;
        }
        let x = position.x - bounds.left();
        let mut top = bounds.top();
        for (i, line) in lines.iter().enumerate() {
            let rows = line.wrap_boundaries().len() + 1;
            let height = line_height * rows as f32;
            if position.y < top + height || i == lines.len() - 1 {
                let row = (((position.y - top) / line_height) as usize).min(rows - 1);
                let y = line_height * (row as f32 + 0.5);
                let ix = index_in_line(line, point(x, y), line_height);
                return self.clamp_to_char_boundary(ranges[i].start + ix);
            }
            top += height;
        }
        self.content.len()
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
        self.preferred_x = None;
        cx.notify()
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
            .char_indices()
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

/// Byte ranges of the hard ('\n'-separated) lines, newline excluded.
/// Always non-empty: "" yields `[0..0]`.
fn hard_line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::with_capacity(1);
    let mut start = 0;
    for (ix, _) in text.match_indices('\n') {
        ranges.push(start..ix);
        start = ix + 1;
    }
    ranges.push(start..text.len());
    ranges
}

/// The hard line containing `offset`. An offset at a line's end (before the
/// '\n') belongs to that line; the position after the '\n' starts the next.
fn line_containing_offset(ranges: &[Range<usize>], offset: usize) -> usize {
    ranges
        .iter()
        .position(|r| offset <= r.end)
        .unwrap_or(ranges.len().saturating_sub(1))
}

/// Total visual rows the element shows: content rows clamped to
/// `min_lines..=max_lines`, never below 1.
fn clamped_row_count(rows: usize, min_lines: usize, max_lines: Option<usize>) -> usize {
    let rows = rows.max(min_lines).max(1);
    match max_lines {
        Some(max) => rows.min(max.max(1)),
        None => rows,
    }
}

/// Intersect a global selection with one visual row of one hard line.
/// Returns line-local `(start, end, extend_past_end)`; `extend_past_end` is
/// true when the selection continues past this hard line (over the '\n'), so
/// the painter widens the quad to keep the selected newline visible.
fn selection_segment(
    selection: &Range<usize>,
    line: &Range<usize>,
    row: &Range<usize>,
    is_last_row: bool,
) -> Option<(usize, usize, bool)> {
    if selection.is_empty() || selection.start > line.end || selection.end < line.start {
        return None;
    }
    let local_start = selection.start.clamp(line.start, line.end) - line.start;
    let local_end = selection.end.clamp(line.start, line.end) - line.start;
    let start = local_start.max(row.start);
    let end = local_end.min(row.end);
    if start > end {
        return None;
    }
    let extend = is_last_row && selection.end > line.end && end == row.end;
    if start == end && !extend {
        return None;
    }
    Some((start, end, extend))
}

/// Line-local byte ranges of each visual (soft-wrapped) row.
fn visual_row_ranges(line: &WrappedLine) -> Vec<Range<usize>> {
    let mut starts = vec![0];
    for boundary in line.wrap_boundaries() {
        let run = &line.unwrapped_layout.runs[boundary.run_ix];
        starts.push(run.glyphs[boundary.glyph_ix].index);
    }
    starts
        .iter()
        .enumerate()
        .map(|(k, &s)| s..starts.get(k + 1).copied().unwrap_or(line.len()))
        .collect()
}

fn index_in_line(line: &WrappedLine, position: Point<Pixels>, line_height: Pixels) -> usize {
    match line.closest_index_for_position(position, line_height) {
        Ok(ix) | Err(ix) => ix,
    }
}

fn line_top(lines: &[WrappedLine], line_ix: usize, line_height: Pixels) -> Pixels {
    lines[..line_ix].iter().fold(px(0.), |acc, line| {
        acc + line_height * (line.wrap_boundaries().len() + 1) as f32
    })
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.preferred_x = None;
        cx.emit(TextAreaEvent::Edited);
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
        // Composition-relative selection offsets into the document by the
        // START of the replaced region (same fix as TextField; the upstream
        // example adds range.end — wrong when the previous marked range was
        // non-empty).
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        self.preferred_x = None;
        cx.emit(TextAreaEvent::Edited);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let lines = self.last_lines.as_ref()?;
        let line_height = self.last_line_height;
        let range = self.range_from_utf16(&range_utf16);
        let ranges = hard_line_ranges(&self.content);
        if lines.len() != ranges.len() {
            return None;
        }
        let line_ix = line_containing_offset(&ranges, range.start);
        let local = range.start.checked_sub(ranges[line_ix].start)?;
        let pos = lines[line_ix].position_for_index(local, line_height)?;
        let top = bounds.top() + line_top(lines, line_ix, line_height) + pos.y;
        // Anchor rect at the range start; enough for IME candidate windows.
        Some(Bounds::new(
            point(bounds.left() + pos.x, top),
            size(px(CURSOR_WIDTH), line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.last_bounds?.localize(&point)?;
        let utf8_index = self.index_for_mouse_position(point);
        Some(self.offset_to_utf16(utf8_index))
    }
}

/// Custom element: shapes the wrapped lines, paints selection/cursor/marked
/// underline, and registers the platform input handler during paint.
struct TextAreaElement {
    input: Entity<TextArea>,
}

struct AreaPrepaint {
    lines: Vec<WrappedLine>,
    line_height: Pixels,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for TextAreaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl TextAreaElement {
    /// `style`/`font_size` are captured while the element's style context is
    /// active — `window.text_style()` inside the layout measure closure would
    /// miss inherited styles.
    fn shape_lines(
        input: &TextArea,
        style: &TextStyle,
        font_size: Pixels,
        wrap_width: Option<Pixels>,
        window: &mut Window,
    ) -> Vec<WrappedLine> {
        let content = input.content.clone();
        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), ThemeText::MUTED)
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
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        window
            .text_system()
            .shape_text(display_text, font_size, &runs, wrap_width, None)
            .map(|lines| lines.into_vec())
            .unwrap_or_default()
    }
}

impl Element for TextAreaElement {
    type RequestLayoutState = ();
    type PrepaintState = AreaPrepaint;

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
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.clone();
        let line_height = window.line_height();
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let layout_id = window.request_measured_layout(
            style,
            move |known_dimensions, available_space, window, cx| {
                let width = known_dimensions.width.or(match available_space.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let entity = input.read(cx);
                let (min_lines, max_lines) = (entity.min_lines, entity.max_lines);
                let lines =
                    Self::shape_lines(input.read(cx), &text_style, font_size, width, window);
                let rows: usize = lines
                    .iter()
                    .map(|line| line.wrap_boundaries().len() + 1)
                    .sum();
                let rows = clamped_row_count(rows, min_lines, max_lines);
                let width = width.unwrap_or_else(|| {
                    lines
                        .iter()
                        .fold(px(0.), |acc, line| acc.max(line.width()))
                });
                size(width, line_height * rows as f32)
            },
        );
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
        let line_height = window.line_height();
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let lines = Self::shape_lines(
            self.input.read(cx),
            &text_style,
            font_size,
            Some(bounds.size.width),
            window,
        );

        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor_offset = input.cursor_offset();

        let mut cursor = None;
        let mut selections = Vec::new();
        if content.is_empty() {
            cursor = Some(fill(
                Bounds::new(bounds.origin, size(px(CURSOR_WIDTH), line_height)),
                Accent::PRIMARY,
            ));
        } else {
            let ranges = hard_line_ranges(&content);
            let cursor_line = line_containing_offset(&ranges, cursor_offset);
            let mut selection_color = Accent::PRIMARY;
            selection_color.a = 0.25;
            let mut top = bounds.top();
            for (i, line) in lines.iter().enumerate() {
                let Some(line_range) = ranges.get(i) else {
                    break;
                };
                let rows = visual_row_ranges(line);
                if selected_range.is_empty() {
                    if i == cursor_line {
                        let local = cursor_offset - line_range.start;
                        if let Some(pos) = line.position_for_index(local, line_height) {
                            cursor = Some(fill(
                                Bounds::new(
                                    point(bounds.left() + pos.x, top + pos.y),
                                    size(px(CURSOR_WIDTH), line_height),
                                ),
                                Accent::PRIMARY,
                            ));
                        }
                    }
                } else {
                    for (k, row) in rows.iter().enumerate() {
                        let Some((start, end, extend)) = selection_segment(
                            &selected_range,
                            line_range,
                            row,
                            k == rows.len() - 1,
                        ) else {
                            continue;
                        };
                        let row_x = line.unwrapped_layout.x_for_index(row.start);
                        let x0 = line.unwrapped_layout.x_for_index(start) - row_x;
                        let mut x1 = line.unwrapped_layout.x_for_index(end) - row_x;
                        if extend {
                            // Keep a selected newline visible (quarter-line nib).
                            x1 += line_height * 0.25;
                        }
                        if x1 > x0 {
                            selections.push(fill(
                                Bounds::from_corners(
                                    point(bounds.left() + x0, top + line_height * k as f32),
                                    point(
                                        bounds.left() + x1,
                                        top + line_height * (k + 1) as f32,
                                    ),
                                ),
                                selection_color,
                            ));
                        }
                    }
                }
                top += line_height * rows.len() as f32;
            }
        }

        AreaPrepaint {
            lines,
            line_height,
            cursor,
            selections,
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
        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }
        let line_height = prepaint.line_height;
        let mut top = bounds.origin.y;
        for line in &prepaint.lines {
            let _ = line.paint(
                point(bounds.origin.x, top),
                line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
            top += line_height * (line.wrap_boundaries().len() + 1) as f32;
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        let lines = std::mem::take(&mut prepaint.lines);
        self.input.update(cx, |input, _cx| {
            input.last_lines = Some(lines);
            input.last_bounds = Some(bounds);
            input.last_line_height = line_height;
        });
    }
}

impl Render for TextArea {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            // Content past max_lines clips instead of painting over
            // neighboring UI (scrolling is still a follow-up).
            .overflow_hidden()
            // "input" gates the "!input" global-shortcut bindings off while
            // any text input is focused.
            .key_context("FrondaTextArea input")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::insert_newline))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .child(TextAreaElement { input: cx.entity() })
    }
}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_line_ranges_basics() {
        assert_eq!(hard_line_ranges(""), vec![0..0]);
        assert_eq!(hard_line_ranges("ab"), vec![0..2]);
        assert_eq!(hard_line_ranges("a\nb"), vec![0..1, 2..3]);
        assert_eq!(hard_line_ranges("a\n"), vec![0..1, 2..2]);
        assert_eq!(hard_line_ranges("\n\n"), vec![0..0, 1..1, 2..2]);
        assert_eq!(hard_line_ranges("\nx"), vec![0..0, 1..2]);
    }

    #[test]
    fn hard_line_ranges_multibyte() {
        // "日本\n語" — 3-byte chars; '\n' at byte 6.
        assert_eq!(hard_line_ranges("日本\n語"), vec![0..6, 7..10]);
    }

    #[test]
    fn line_containing_offset_boundaries() {
        let ranges = hard_line_ranges("ab\ncd\n\nef");
        // "ab" 0..2, "cd" 3..5, "" 6..6, "ef" 7..9
        assert_eq!(line_containing_offset(&ranges, 0), 0);
        assert_eq!(line_containing_offset(&ranges, 2), 0); // end of line 0
        assert_eq!(line_containing_offset(&ranges, 3), 1); // start of line 1
        assert_eq!(line_containing_offset(&ranges, 5), 1);
        assert_eq!(line_containing_offset(&ranges, 6), 2); // empty line
        assert_eq!(line_containing_offset(&ranges, 7), 3);
        assert_eq!(line_containing_offset(&ranges, 9), 3);
        // Past-the-end clamps to the last line.
        assert_eq!(line_containing_offset(&ranges, 99), 3);
    }

    #[test]
    fn clamped_row_count_bounds() {
        assert_eq!(clamped_row_count(0, 1, None), 1);
        assert_eq!(clamped_row_count(2, 5, None), 5);
        assert_eq!(clamped_row_count(7, 5, None), 7);
        assert_eq!(clamped_row_count(20, 5, Some(8)), 8);
        assert_eq!(clamped_row_count(3, 0, None), 3);
        assert_eq!(clamped_row_count(0, 0, None), 1);
        // max below min: the cap wins.
        assert_eq!(clamped_row_count(1, 5, Some(3)), 3);
    }

    #[test]
    fn selection_segment_single_line() {
        let line = 0..5;
        let row = 0..5;
        assert_eq!(selection_segment(&(1..3), &line, &row, true), Some((1, 3, false)));
        // Empty selection: nothing.
        assert_eq!(selection_segment(&(2..2), &line, &row, true), None);
        // Selection fully outside the line.
        assert_eq!(selection_segment(&(7..9), &line, &row, true), None);
    }

    #[test]
    fn selection_segment_spanning_newline() {
        // "ab\ncd": selecting 1..4 covers "b", the newline, and "c".
        let line0 = 0..2;
        let line1 = 3..5;
        let row0 = 0..2;
        let row1 = 0..2;
        // Line 0: local 1..2, extends over the newline.
        assert_eq!(selection_segment(&(1..4), &line0, &row0, true), Some((1, 2, true)));
        // Line 1: local 0..1, no extension.
        assert_eq!(selection_segment(&(1..4), &line1, &row1, true), Some((0, 1, false)));
    }

    #[test]
    fn selection_segment_empty_line_inside_selection() {
        // "a\n\nb": the empty middle line (2..2) inside selection 0..4.
        let line = 2..2;
        let row = 0..0;
        assert_eq!(selection_segment(&(0..4), &line, &row, true), Some((0, 0, true)));
        // Selection ending exactly at the empty line does not extend.
        assert_eq!(selection_segment(&(0..2), &line, &row, true), None);
    }

    #[test]
    fn selection_segment_soft_wrapped_rows() {
        // One hard line 0..10, soft-wrapped into rows 0..4 and 4..10.
        let line = 0..10;
        let row0 = 0..4;
        let row1 = 4..10;
        let sel = 2..7;
        assert_eq!(selection_segment(&sel, &line, &row0, false), Some((2, 4, false)));
        assert_eq!(selection_segment(&sel, &line, &row1, true), Some((4, 7, false)));
        // Selection endpoint exactly on the wrap boundary produces no
        // zero-width duplicate on the next row.
        let sel = 2..4;
        assert_eq!(selection_segment(&sel, &line, &row0, false), Some((2, 4, false)));
        assert_eq!(selection_segment(&sel, &line, &row1, true), None);
    }
}
