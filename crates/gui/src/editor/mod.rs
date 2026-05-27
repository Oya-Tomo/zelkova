pub mod highlight;
pub mod ime;
pub mod input;
pub mod render;
pub mod util;

pub use highlight::{
    HighlightedLine, ResolvedColors, detect_line_context, highlight_fence_line, highlight_line,
    parse_hex,
};
pub use ime::ImeState;
pub use zelkova_rope::Buffer;

use std::ops::Range;
use std::path::PathBuf;

use chrono::Utc;
use gpui::{
    App, Context, ElementInputHandler, FocusHandle, Focusable, IntoElement, Render, ScrollHandle,
    SharedString, StyledText, Window, div, prelude::*, px,
};
use zelkova_config::{EditorColors, UiColors};
use zelkova_note_core::{Frontmatter, format_note_file, parse_note_content};

use crate::{
    Backspace, InsertNewline, MoveDown, MoveLeft, MoveRight, MoveUp, Redo, SaveNote, SelectAll,
    SelectDown, SelectLeft, SelectRight, SelectUp, Undo,
};
use util::{char_idx_to_byte, parse_tags_from_input, pixel_to_col, split_at_char_col, split_lines};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum EditZone {
    Title,
    TagInput,
    Content,
}

pub struct Editor {
    pub(super) focus_handle: FocusHandle,
    pub(super) buffer: Buffer,
    pub(super) cached_text: String,
    pub(super) cached_lines: Vec<String>,
    pub(super) cursor_pos: usize,
    pub(super) selection: Option<Range<usize>>,
    pub(super) ime_state: ImeState,
    pub(super) file_path: Option<PathBuf>,
    socket_path: Option<PathBuf>,
    pub(super) resolved_colors: ResolvedColors,
    pub(super) dirty: bool,
    pub(super) frontmatter: Option<Frontmatter>,
    pub(super) tag_input: String,
    pub(super) tag_input_cursor: usize,
    pub(super) edit_zone: EditZone,
    pub(super) title_cursor: usize,
    pub(super) cached_highlights: Vec<HighlightedLine>,
    pub(super) highlights_dirty: bool,
    pub(super) dragging: bool,
    pub(super) scroll_handle: ScrollHandle,
    wrap: bool,
}

impl Editor {
    pub fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            buffer: Buffer::new(),
            cached_text: String::new(),
            cached_lines: vec![String::new()],
            cursor_pos: 0,
            selection: None,
            ime_state: ImeState::new(),
            file_path: None,
            socket_path: None,
            resolved_colors: ResolvedColors::new(&EditorColors::default(), &UiColors::default()),
            dirty: false,
            frontmatter: None,
            tag_input: String::new(),
            tag_input_cursor: 0,
            edit_zone: EditZone::Content,
            title_cursor: 0,
            cached_highlights: Vec::new(),
            highlights_dirty: false,
            dragging: false,
            scroll_handle: ScrollHandle::new(),
            wrap: true,
        }
    }

    pub fn load(path: PathBuf, cx: &mut App) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(&path)?;
        let (frontmatter, body) = match parse_note_content(&raw) {
            (Some(fm), body) => (Some(fm), body),
            (None, _) => (None, raw),
        };
        let cached_lines = split_lines(&body);
        let edit_zone = match &frontmatter {
            Some(fm) if fm.title.is_empty() => EditZone::Title,
            _ => EditZone::Content,
        };
        Ok(Self {
            focus_handle: cx.focus_handle(),
            buffer: Buffer::from(&body),
            cached_text: body,
            cached_lines,
            cursor_pos: 0,
            selection: None,
            ime_state: ImeState::new(),
            file_path: Some(path),
            socket_path: None,
            resolved_colors: ResolvedColors::new(&EditorColors::default(), &UiColors::default()),
            dirty: false,
            frontmatter,
            tag_input: String::new(),
            tag_input_cursor: 0,
            edit_zone,
            title_cursor: 0,
            cached_highlights: Vec::new(),
            highlights_dirty: false,
            dragging: false,
            scroll_handle: ScrollHandle::new(),
            wrap: true,
        })
    }

    pub fn set_socket_path(&mut self, path: PathBuf) {
        self.socket_path = Some(path);
    }

    #[allow(dead_code)]
    pub fn set_theme(&mut self, theme: EditorColors, ui: &UiColors) {
        self.resolved_colors = ResolvedColors::new(&theme, ui);
        self.highlights_dirty = true;
    }

    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
    }

    pub(super) fn scroll_to_cursor(&self, header_count: usize) {
        if self.edit_zone != EditZone::Content {
            return;
        }
        let (cursor_line, _) = self.byte_to_line_col(self.cursor_pos);
        self.scroll_handle
            .scroll_to_item(cursor_line + header_count);
    }

    pub fn text(&self) -> &str {
        &self.cached_text
    }

    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[allow(dead_code)]
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub fn title(&self) -> &str {
        self.frontmatter
            .as_ref()
            .map(|f| f.title.as_str())
            .unwrap_or("Untitled")
    }

    #[allow(dead_code)]
    pub fn tags(&self) -> Vec<&str> {
        self.frontmatter
            .as_ref()
            .map(|f| f.tags.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn add_tag(&mut self, tag: String) {
        if let Some(fm) = &mut self.frontmatter {
            fm.tags.insert(tag);
            self.dirty = true;
        }
    }

    pub fn remove_tag(&mut self, tag: &str) {
        if let Some(fm) = &mut self.frontmatter {
            fm.tags.remove(tag);
            self.dirty = true;
        }
    }

    /// Populate tag_input with existing frontmatter tags as "#xxx #yyy ".
    pub(super) fn populate_tag_input(&mut self) {
        if let Some(fm) = &self.frontmatter {
            if fm.tags.is_empty() {
                self.tag_input.clear();
            } else {
                let mut s = String::new();
                for tag in &fm.tags {
                    s.push('#');
                    s.push_str(tag);
                    s.push(' ');
                }
                self.tag_input = s;
            }
            self.tag_input_cursor = self.tag_input.chars().count();
        }
    }

    /// Parse #xxx tokens from tag input, normalize full-width spaces,
    /// update frontmatter tags, and clear the input field.
    /// Returns the cursor position before clearing.
    pub(super) fn commit_tag_input(&mut self) -> usize {
        let saved_cursor = self.tag_input_cursor;
        if !self.tag_input.is_empty() {
            let normalized = self.tag_input.replace('\u{3000}', " ");
            let parsed = parse_tags_from_input(&normalized);
            if let Some(fm) = &mut self.frontmatter {
                fm.tags = parsed;
                self.dirty = true;
            }
        }
        self.tag_input.clear();
        self.tag_input_cursor = 0;
        saved_cursor
    }

    // --- Line helpers using cached_lines ---

    pub(super) fn rebuild_lines(&mut self) {
        self.cached_lines = split_lines(&self.cached_text);
        self.highlights_dirty = true;
    }

    pub(super) fn line_count(&self) -> usize {
        self.cached_lines.len()
    }

    pub(super) fn line_text(&self, idx: usize) -> &str {
        self.cached_lines.get(idx).map(|s| s.as_str()).unwrap_or("")
    }

    /// byte offset → (line_index, char_column)
    pub(super) fn byte_to_line_col(&self, byte_pos: usize) -> (usize, usize) {
        let text = &self.cached_text;
        let mut current_byte = 0;
        for (line_idx, line) in text.lines().enumerate() {
            let line_end = current_byte + line.len();
            if byte_pos <= line_end {
                let col = text[current_byte..byte_pos].chars().count();
                return (line_idx, col);
            }
            current_byte = line_end + 1; // skip \n
        }
        // Past all lines (cursor at very end of text, or after trailing \n)
        let last = self.cached_lines.len().saturating_sub(1);
        (last, 0)
    }

    /// (line_index, char_column) → byte offset
    pub(super) fn line_col_to_byte(&self, line: usize, col: usize) -> usize {
        let text = &self.cached_text;
        let mut current_byte = 0;
        for (line_idx, line_text) in text.lines().enumerate() {
            if line_idx == line {
                let mut char_count = 0;
                for (byte_idx, _) in line_text.char_indices() {
                    if char_count == col {
                        return current_byte + byte_idx;
                    }
                    char_count += 1;
                }
                return current_byte + line_text.len();
            }
            current_byte += line_text.len() + 1;
        }
        text.len()
    }

    /// Incremental cache update: applies the same edit to cached_text.
    /// Avoids calling buffer.text() (O(n) Rope traversal) on every keystroke.
    pub(super) fn cache_edit(&mut self, start: usize, end: usize, new_text: &str) {
        self.cached_text.replace_range(start..end, new_text);
        self.rebuild_lines();
    }

    pub(super) fn invalidate_cache(&mut self) {
        self.cached_text = self.buffer.text();
        self.rebuild_lines();
    }

    // --- Action handlers ---

    fn handle_move_left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(sel) = self.selection.take() {
            self.cursor_pos = sel.start;
            cx.notify();
            return;
        }
        match self.edit_zone {
            EditZone::Title => {
                if self.title_cursor > 0 {
                    self.title_cursor -= 1;
                    cx.notify();
                }
            }
            EditZone::TagInput => {
                if self.tag_input_cursor > 0 {
                    self.tag_input_cursor -= 1;
                    cx.notify();
                }
            }
            EditZone::Content => {
                if self.cursor_pos > 0 {
                    let prev_len = self.cached_text[..self.cursor_pos]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.cursor_pos -= prev_len;
                    cx.notify();
                }
            }
        }
    }

    fn handle_move_right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(sel) = self.selection.take() {
            self.cursor_pos = sel.end;
            cx.notify();
            return;
        }
        match self.edit_zone {
            EditZone::Title => {
                let title_len = self
                    .frontmatter
                    .as_ref()
                    .map(|f| f.title.chars().count())
                    .unwrap_or(0);
                if self.title_cursor < title_len {
                    self.title_cursor += 1;
                    cx.notify();
                }
            }
            EditZone::TagInput => {
                let len = self.tag_input.chars().count();
                if self.tag_input_cursor < len {
                    self.tag_input_cursor += 1;
                    cx.notify();
                }
            }
            EditZone::Content => {
                if self.cursor_pos < self.cached_text.len() {
                    let next_len = self.cached_text[self.cursor_pos..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    self.cursor_pos += next_len;
                    cx.notify();
                }
            }
        }
    }

    // --- Selection handlers (Shift+Arrow) ---

    fn handle_select_left(&mut self, _: &SelectLeft, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_zone != EditZone::Content {
            return;
        }
        if self.cursor_pos > 0 {
            let prev_len = self.cached_text[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            let new_pos = self.cursor_pos - prev_len;
            self.extend_selection(new_pos);
            cx.notify();
        }
    }

    fn handle_select_right(
        &mut self,
        _: &SelectRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit_zone != EditZone::Content {
            return;
        }
        if self.cursor_pos < self.cached_text.len() {
            let next_len = self.cached_text[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            let new_pos = self.cursor_pos + next_len;
            self.extend_selection(new_pos);
            cx.notify();
        }
    }

    fn handle_select_up(&mut self, _: &SelectUp, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_zone != EditZone::Content {
            return;
        }
        let (line, col) = self.byte_to_line_col(self.cursor_pos);
        if line > 0 {
            let new_pos = self.line_col_to_byte(line - 1, col);
            self.extend_selection(new_pos);
            cx.notify();
        }
    }

    fn handle_select_down(&mut self, _: &SelectDown, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_zone != EditZone::Content {
            return;
        }
        let (line, col) = self.byte_to_line_col(self.cursor_pos);
        let total_lines = self.line_count();
        if line + 1 < total_lines {
            let new_pos = self.line_col_to_byte(line + 1, col);
            self.extend_selection(new_pos);
            cx.notify();
        }
    }

    /// Extend or start a selection from cursor_pos to new_pos.
    /// The anchor is the OPPOSITE end of the selection from the cursor.
    pub(super) fn extend_selection(&mut self, new_pos: usize) {
        let anchor = match &self.selection {
            Some(sel) => {
                // cursor is at sel.start → anchor is sel.end, and vice versa
                if self.cursor_pos == sel.start {
                    sel.end
                } else {
                    sel.start
                }
            }
            None => self.cursor_pos,
        };
        self.cursor_pos = new_pos;
        self.selection = Some(anchor.min(new_pos)..anchor.max(new_pos));
    }

    fn handle_select_all(&mut self, _: &SelectAll, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_zone != EditZone::Content {
            return;
        }
        let len = self.cached_text.len();
        if len > 0 {
            self.cursor_pos = len;
            self.selection = Some(0..len);
            cx.notify();
        }
    }

    pub fn handle_copy(&mut self, _: &crate::Copy, _window: &mut Window, cx: &mut Context<Self>) {
        let text = match self.edit_zone {
            EditZone::Title => self
                .frontmatter
                .as_ref()
                .map(|fm| fm.title.clone())
                .unwrap_or_default(),
            EditZone::TagInput => self.tag_input.clone(),
            EditZone::Content => {
                if let Some(ref sel) = self.selection {
                    let start = sel.start.min(self.cached_text.len());
                    let end = sel.end.min(self.cached_text.len());
                    self.cached_text[start..end].to_string()
                } else {
                    return;
                }
            }
        };
        if !text.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    pub fn handle_paste(&mut self, _: &crate::Paste, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        let text = item.text().unwrap_or_default().to_string();
        if text.is_empty() {
            return;
        }
        match self.edit_zone {
            EditZone::Title => {
                if let Some(fm) = &mut self.frontmatter {
                    let byte_pos = char_idx_to_byte(&fm.title, self.title_cursor);
                    fm.title.insert_str(byte_pos, &text);
                    self.title_cursor += text.chars().count();
                    self.dirty = true;
                    cx.notify();
                }
            }
            EditZone::TagInput => {
                let byte_pos = char_idx_to_byte(&self.tag_input, self.tag_input_cursor);
                self.tag_input.insert_str(byte_pos, &text);
                self.tag_input_cursor += text.chars().count();
                cx.notify();
            }
            EditZone::Content => {
                if let Some(sel) = self.selection.take() {
                    self.buffer.delete(sel.start, sel.end);
                    self.cache_edit(sel.start, sel.end, "");
                    self.cursor_pos = sel.start;
                }
                self.buffer.insert(self.cursor_pos, &text);
                self.cache_edit(self.cursor_pos, self.cursor_pos, &text);
                self.cursor_pos += text.len();
                self.dirty = true;
                cx.notify();
            }
        }
    }

    pub fn handle_cut(&mut self, _: &crate::Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.handle_copy(&crate::Copy, window, cx);
        match self.edit_zone {
            EditZone::Title => {
                if self.title_cursor > 0 {
                    if let Some(fm) = &mut self.frontmatter {
                        let byte_pos = char_idx_to_byte(&fm.title, self.title_cursor);
                        let prev_len = fm.title[..byte_pos]
                            .chars()
                            .last()
                            .map(|c| c.len_utf8())
                            .unwrap_or(0);
                        if prev_len > 0 {
                            fm.title.drain((byte_pos - prev_len)..byte_pos);
                            self.title_cursor -= 1;
                            self.dirty = true;
                        }
                    }
                    cx.notify();
                }
            }
            EditZone::TagInput => {
                if self.tag_input_cursor > 0 {
                    let byte_pos = char_idx_to_byte(&self.tag_input, self.tag_input_cursor);
                    let prev_len = self.tag_input[..byte_pos]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    if prev_len > 0 {
                        self.tag_input.drain((byte_pos - prev_len)..byte_pos);
                        self.tag_input_cursor -= 1;
                    }
                    cx.notify();
                }
            }
            EditZone::Content => {
                if let Some(sel) = self.selection.take() {
                    self.buffer.delete(sel.start, sel.end);
                    self.cache_edit(sel.start, sel.end, "");
                    self.cursor_pos = sel.start;
                    self.dirty = true;
                    cx.notify();
                } else if self.cursor_pos > 0 {
                    let prev_len = self.cached_text[..self.cursor_pos]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    let start = self.cursor_pos - prev_len;
                    self.buffer.delete(start, self.cursor_pos);
                    self.cache_edit(start, self.cursor_pos, "");
                    self.cursor_pos = start;
                    self.dirty = true;
                    cx.notify();
                }
            }
        }
    }

    fn handle_move_up(&mut self, _: &MoveUp, _window: &mut Window, cx: &mut Context<Self>) {
        self.selection = None;
        match self.edit_zone {
            EditZone::Title => {}
            EditZone::TagInput => {
                let saved_cursor = self.commit_tag_input();
                self.edit_zone = EditZone::Title;
                let title_len = self
                    .frontmatter
                    .as_ref()
                    .map(|f| f.title.chars().count())
                    .unwrap_or(0);
                self.title_cursor = saved_cursor.min(title_len);
                cx.notify();
            }
            EditZone::Content => {
                let (line, col) = self.byte_to_line_col(self.cursor_pos);
                if line == 0 && self.frontmatter.is_some() {
                    self.edit_zone = EditZone::TagInput;
                    self.populate_tag_input();
                    self.tag_input_cursor = col.min(self.tag_input.chars().count());
                    cx.notify();
                } else if line > 0 {
                    self.cursor_pos = self.line_col_to_byte(line - 1, col);
                    cx.notify();
                }
            }
        }
    }

    fn handle_move_down(&mut self, _: &MoveDown, _window: &mut Window, cx: &mut Context<Self>) {
        self.selection = None;
        let total_lines = self.line_count();
        match self.edit_zone {
            EditZone::Title => {
                self.edit_zone = EditZone::TagInput;
                self.populate_tag_input();
                self.tag_input_cursor = self.title_cursor.min(self.tag_input.chars().count());
                cx.notify();
            }
            EditZone::TagInput => {
                let saved_cursor = self.commit_tag_input();
                self.edit_zone = EditZone::Content;
                self.cursor_pos = self.line_col_to_byte(0, saved_cursor);
                cx.notify();
            }
            EditZone::Content => {
                let (line, col) = self.byte_to_line_col(self.cursor_pos);
                if line + 1 < total_lines {
                    self.cursor_pos = self.line_col_to_byte(line + 1, col);
                    cx.notify();
                }
            }
        }
    }

    fn handle_backspace(&mut self, _: &Backspace, _window: &mut Window, cx: &mut Context<Self>) {
        match self.edit_zone {
            EditZone::Title => {
                if self.title_cursor > 0 {
                    if let Some(fm) = &mut self.frontmatter {
                        let byte_pos = char_idx_to_byte(&fm.title, self.title_cursor);
                        let prev_len = fm.title[..byte_pos]
                            .chars()
                            .last()
                            .map(|c| c.len_utf8())
                            .unwrap_or(0);
                        if prev_len > 0 {
                            fm.title.drain((byte_pos - prev_len)..byte_pos);
                            self.title_cursor -= 1;
                            self.dirty = true;
                        }
                    }
                    cx.notify();
                }
            }
            EditZone::TagInput => {
                if self.tag_input_cursor > 0 {
                    let byte_pos = char_idx_to_byte(&self.tag_input, self.tag_input_cursor);
                    let prev_len = self.tag_input[..byte_pos]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                    if prev_len > 0 {
                        self.tag_input.drain((byte_pos - prev_len)..byte_pos);
                        self.tag_input_cursor -= 1;
                    }
                    cx.notify();
                }
            }
            EditZone::Content => {
                // If there's a selection, delete it
                if let Some(sel) = self.selection.take() {
                    self.buffer.delete(sel.start, sel.end);
                    self.cache_edit(sel.start, sel.end, "");
                    self.cursor_pos = sel.start;
                    self.dirty = true;
                    cx.notify();
                    return;
                }
                if self.cursor_pos > 0 {
                    let prev_len = self.cached_text[..self.cursor_pos]
                        .chars()
                        .last()
                        .map(|c| c.len_utf8())
                        .unwrap_or(1);
                    let start = self.cursor_pos - prev_len;
                    self.buffer.delete(start, self.cursor_pos);
                    self.cache_edit(start, self.cursor_pos, "");
                    self.cursor_pos = start;
                    self.dirty = true;
                    cx.notify();
                }
            }
        }
    }

    fn handle_enter(&mut self, _: &InsertNewline, _window: &mut Window, cx: &mut Context<Self>) {
        match self.edit_zone {
            EditZone::Title => {
                self.edit_zone = EditZone::Content;
                self.cursor_pos = 0;
                cx.notify();
            }
            EditZone::TagInput => {
                let saved_cursor = self.commit_tag_input();
                self.edit_zone = EditZone::Content;
                self.cursor_pos = self.line_col_to_byte(0, saved_cursor);
                cx.notify();
            }
            EditZone::Content => {
                // Delete selection if present
                if let Some(sel) = self.selection.take() {
                    self.buffer.delete(sel.start, sel.end);
                    self.cache_edit(sel.start, sel.end, "");
                    self.cursor_pos = sel.start;
                }
                self.buffer.insert(self.cursor_pos, "\n");
                self.cache_edit(self.cursor_pos, self.cursor_pos, "\n");
                self.cursor_pos += 1;
                self.dirty = true;
                cx.notify();
            }
        }
    }

    fn handle_save(&mut self, _: &SaveNote, _window: &mut Window, cx: &mut Context<Self>) {
        if self.edit_zone == EditZone::TagInput {
            self.commit_tag_input();
        }
        self.save_to_disk();
        cx.notify();
    }

    fn handle_undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.undo() {
            self.invalidate_cache();
            if self.cursor_pos > self.cached_text.len() {
                self.cursor_pos = self.cached_text.len();
            }
            cx.notify();
        }
    }

    fn handle_redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
        if self.buffer.redo() {
            self.invalidate_cache();
            if self.cursor_pos > self.cached_text.len() {
                self.cursor_pos = self.cached_text.len();
            }
            cx.notify();
        }
    }

    fn save_to_disk(&mut self) {
        if let Some(path) = &self.file_path {
            let content = if let Some(fm) = &mut self.frontmatter {
                fm.updated = Utc::now();
                format_note_file(fm, &self.cached_text)
            } else {
                self.cached_text.clone()
            };
            if std::fs::write(path, content).is_ok() {
                self.dirty = false;
                self.notify_daemon();
            }
        }
    }

    fn notify_daemon(&self) {
        if let (Some(socket), Some(path)) = (&self.socket_path, &self.file_path) {
            if !socket.exists() {
                return;
            }
            let client = zelkova_rpc::client::RpcClient::new(socket);
            if let Err(e) = client.note_updated(path) {
                eprintln!("warning: failed to notify daemon of note update: {e}");
            }
        }
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lines = &self.cached_lines;
        let (cursor_line, cursor_col) = self.byte_to_line_col(self.cursor_pos);
        let ascii_char_width = 7.2_f32;

        // --- Frontmatter header ---
        let header_children = self.render_frontmatter_header(cx);

        // --- Content lines ---
        let mut children = Vec::new();

        // Lazy highlight: rebuild only when dirty. First render shows plain text,
        // then highlights are built and displayed on next frame.
        if self.highlights_dirty {
            self.cached_highlights = render::build_highlights(lines, &self.resolved_colors);
            self.highlights_dirty = false;
        }

        let has_highlights = !self.cached_highlights.is_empty();

        for (line_idx, line_text) in lines.iter().enumerate() {
            let display_text = if line_text.is_empty() {
                " ".to_string()
            } else {
                line_text.clone()
            };

            let mut line_div = div()
                .w_full()
                .when(!self.wrap, |el| el.h(px(22.0)))
                .when(self.wrap, |el| el.min_h(px(22.0)).whitespace_normal())
                .flex()
                .flex_row()
                .items_start()
                .cursor(gpui::CursorStyle::IBeam)
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                        this.edit_zone = EditZone::Content;
                        this.dragging = true;
                        let click_line = line_idx;
                        let line_text = this.line_text(click_line);
                        let click_col = pixel_to_col(line_text, event.position.x, ascii_char_width);
                        this.cursor_pos = this.line_col_to_byte(click_line, click_col);
                        this.selection = None;
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(
                    move |this, event: &gpui::MouseMoveEvent, _window, cx| {
                        if !this.dragging {
                            return;
                        }
                        let move_line = line_idx;
                        let line_text = this.line_text(move_line);
                        let move_col = pixel_to_col(line_text, event.position.x, ascii_char_width);
                        let new_pos = this.line_col_to_byte(move_line, move_col);
                        this.extend_selection(new_pos);
                        cx.notify();
                    },
                ));

            if has_highlights {
                line_div = self.render_highlighted_line(
                    line_idx,
                    line_text,
                    display_text,
                    line_div,
                    cursor_line,
                    cursor_col,
                );
            } else {
                // Plain text — fast path, no highlight processing
                if line_idx == cursor_line && self.edit_zone == EditZone::Content {
                    let (before, after) = split_at_char_col(&display_text, cursor_col);
                    line_div = line_div
                        .child(StyledText::new(SharedString::from(before)))
                        .child(
                            div()
                                .w(px(2.0))
                                .h(px(18.0))
                                .bg(self.resolved_colors.text)
                                .flex_shrink_0(),
                        )
                        .child(StyledText::new(if after.is_empty() {
                            SharedString::from(" ")
                        } else {
                            SharedString::from(after)
                        }));
                } else {
                    line_div = line_div.child(StyledText::new(SharedString::from(display_text)));
                }
            }

            children.push(line_div.into_any_element());
        }

        // Insert image rows below lines that contain images.
        // Group consecutive lines with images so their images display side-by-side.
        {
            let line_count = lines.len();
            // Build a list of (insert_after_idx, image_urls) for consecutive groups
            let mut groups: Vec<(usize, Vec<String>)> = Vec::new();
            let mut i = 0;
            while i < line_count {
                let urls: Vec<String> = if has_highlights {
                    self.cached_highlights
                        .get(i)
                        .map(|h| h.image_urls.clone())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                if urls.is_empty() {
                    i += 1;
                    continue;
                }
                // Found start of a consecutive image group
                let mut group_urls = Vec::new();
                let group_start = i;
                while i < line_count {
                    let line_urls: Vec<String> = if has_highlights {
                        self.cached_highlights
                            .get(i)
                            .map(|h| h.image_urls.clone())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    if line_urls.is_empty() {
                        break;
                    }
                    group_urls.extend(line_urls);
                    i += 1;
                }
                // Insert after the last line of this group (index in children = group_start + offset)
                groups.push((group_start + (i - group_start), group_urls));
            }

            // Insert image rows in reverse order so indices stay valid
            for (insert_after, urls) in groups.into_iter().rev() {
                if insert_after <= children.len() {
                    children.insert(insert_after, self.render_image_row(&urls));
                }
            }
        }

        // Schedule highlight build for next frame if not yet done
        if !has_highlights && !lines.is_empty() {
            self.highlights_dirty = true;
            cx.notify();
        }

        // Register input handler when focused
        if self.focus_handle.is_focused(window) {
            window.handle_input(
                &self.focus_handle,
                ElementInputHandler::new(gpui::Bounds::default(), cx.entity()),
                cx,
            );
        }

        let header_count = header_children.len();
        self.scroll_to_cursor(header_count);

        // Inner div takes natural height from children, allowing the outer
        // scroll container to detect overflow and enable scrolling.
        let content_div = div()
            .flex()
            .flex_col()
            .children(header_children)
            .children(children);

        div()
            .id("editor-scroll")
            .size_full()
            .when(self.wrap, |el| el.overflow_y_scroll())
            .when(!self.wrap, |el| el.overflow_scroll())
            .track_scroll(&self.scroll_handle)
            .track_focus(&self.focus_handle)
            .text_color(self.resolved_colors.text)
            .text_sm()
            .font_family("monospace")
            .p(px(8.0))
            .child(content_div)
            .on_action(cx.listener(Editor::handle_move_left))
            .on_action(cx.listener(Editor::handle_move_right))
            .on_action(cx.listener(Editor::handle_move_up))
            .on_action(cx.listener(Editor::handle_move_down))
            .on_action(cx.listener(Editor::handle_select_left))
            .on_action(cx.listener(Editor::handle_select_right))
            .on_action(cx.listener(Editor::handle_select_up))
            .on_action(cx.listener(Editor::handle_select_down))
            .on_action(cx.listener(Editor::handle_backspace))
            .on_action(cx.listener(Editor::handle_enter))
            .on_action(cx.listener(Editor::handle_undo))
            .on_action(cx.listener(Editor::handle_redo))
            .on_action(cx.listener(Editor::handle_save))
            .on_action(cx.listener(Editor::handle_select_all))
            .on_action(cx.listener(Editor::handle_copy))
            .on_action(cx.listener(Editor::handle_paste))
            .on_action(cx.listener(Editor::handle_cut))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, _window, _cx| {
                    this.dragging = false;
                }),
            )
    }
}
