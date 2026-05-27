pub mod highlight;
pub mod ime;

pub use highlight::{
    BlockContext, HighlightedLine, ResolvedColors, detect_line_context, highlight_fence_line,
    highlight_line, parse_hex,
};
pub use ime::ImeState;
pub use zelkova_rope::Buffer;

use std::ops::Range;
use std::path::PathBuf;

use chrono::Utc;
use gpui::{
    App, Bounds, Context, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable,
    FontWeight, HighlightStyle, IntoElement, Pixels, Point, Render, SharedString, StyledText,
    UTF16Selection, Window, div, img, prelude::*, px, rgb,
};
use zelkova_config::EditorColors;
use zelkova_note_core::Frontmatter;

use crate::{
    Backspace, InsertNewline, MoveDown, MoveLeft, MoveRight, MoveUp, Redo, SaveNote, SelectAll,
    SelectDown, SelectLeft, SelectRight, SelectUp, Undo,
};

#[derive(Clone, Copy, PartialEq)]
enum EditZone {
    Title,
    TagInput,
    Content,
}

pub struct Editor {
    focus_handle: FocusHandle,
    buffer: Buffer,
    cached_text: String,
    cached_lines: Vec<String>,
    cursor_pos: usize,
    selection: Option<Range<usize>>,
    ime_state: ImeState,
    file_path: Option<PathBuf>,
    socket_path: Option<PathBuf>,
    resolved_colors: ResolvedColors,
    dirty: bool,
    frontmatter: Option<Frontmatter>,
    tag_input: String,
    tag_input_cursor: usize,
    edit_zone: EditZone,
    title_cursor: usize,
    cached_highlights: Vec<HighlightedLine>,
    highlights_dirty: bool,
    dragging: bool,
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
            resolved_colors: ResolvedColors::new(&EditorColors::default()),
            dirty: false,
            frontmatter: None,
            tag_input: String::new(),
            tag_input_cursor: 0,
            edit_zone: EditZone::Content,
            title_cursor: 0,
            cached_highlights: Vec::new(),
            highlights_dirty: false,
            dragging: false,
        }
    }

    pub fn load(path: PathBuf, cx: &mut App) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(&path)?;
        let (frontmatter, body) = match parse_frontmatter_gui(&raw) {
            Some(result) => result,
            None => (None, raw),
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
            resolved_colors: ResolvedColors::new(&EditorColors::default()),
            dirty: false,
            frontmatter,
            tag_input: String::new(),
            tag_input_cursor: 0,
            edit_zone,
            title_cursor: 0,
            cached_highlights: Vec::new(),
            highlights_dirty: false,
            dragging: false,
        })
    }

    pub fn set_socket_path(&mut self, path: PathBuf) {
        self.socket_path = Some(path);
    }

    pub fn set_theme(&mut self, theme: EditorColors) {
        self.resolved_colors = ResolvedColors::new(&theme);
        self.highlights_dirty = true;
    }

    pub fn text(&self) -> &str {
        &self.cached_text
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub fn title(&self) -> &str {
        self.frontmatter
            .as_ref()
            .map(|f| f.title.as_str())
            .unwrap_or("Untitled")
    }

    pub fn tags(&self) -> Vec<&str> {
        self.frontmatter
            .as_ref()
            .map(|f| f.tags.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

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
    fn populate_tag_input(&mut self) {
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
    fn commit_tag_input(&mut self) -> usize {
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

    fn rebuild_lines(&mut self) {
        self.cached_lines = split_lines(&self.cached_text);
        self.highlights_dirty = true;
    }

    fn line_count(&self) -> usize {
        self.cached_lines.len()
    }

    fn line_text(&self, idx: usize) -> &str {
        self.cached_lines.get(idx).map(|s| s.as_str()).unwrap_or("")
    }

    /// byte offset → (line_index, char_column)
    fn byte_to_line_col(&self, byte_pos: usize) -> (usize, usize) {
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
    fn line_col_to_byte(&self, line: usize, col: usize) -> usize {
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
    fn cache_edit(&mut self, start: usize, end: usize, new_text: &str) {
        self.cached_text.replace_range(start..end, new_text);
        self.rebuild_lines();
    }

    fn invalidate_cache(&mut self) {
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
    fn extend_selection(&mut self, new_pos: usize) {
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
            let _ = client.note_updated(path);
        }
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// --- Highlighted line rendering (extracted from render) ---

impl Editor {
    fn render_frontmatter_header(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let mut children = Vec::new();

        let Some(fm) = &self.frontmatter else {
            return children;
        };

        let title = fm.title.clone();
        let tags: Vec<String> = fm.tags.iter().cloned().collect();
        let created = fm.created.format("%Y-%m-%d %H:%M").to_string();
        let updated = fm.updated.format("%Y-%m-%d %H:%M").to_string();

        let is_title_zone = self.edit_zone == EditZone::Title;
        let tc = self.title_cursor;
        let title_for_cursor = title.clone();
        let title_div = {
            let mut container = div()
                .w_full()
                .py(px(4.0))
                .flex()
                .flex_row()
                .items_center()
                .cursor(gpui::CursorStyle::IBeam)
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _ev, _window, cx| {
                        this.edit_zone = EditZone::Title;
                        this.selection = None;
                        this.title_cursor = this
                            .frontmatter
                            .as_ref()
                            .map(|f| f.title.chars().count())
                            .unwrap_or(0);
                        cx.notify();
                    }),
                );

            if is_title_zone {
                let (before, after) = split_at_char_col(&title_for_cursor, tc);
                container = container
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0xcdd6f4))
                            .child(before),
                    )
                    .child(
                        div()
                            .w(px(2.0))
                            .h(px(24.0))
                            .bg(rgb(0xcdd6f4))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(0xcdd6f4))
                            .child(if after.is_empty() {
                                " ".to_string()
                            } else {
                                after
                            }),
                    );
            } else {
                let title_color = if title.is_empty() {
                    rgb(0xa6adc8)
                } else {
                    rgb(0xcdd6f4)
                };
                container = container.child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(title_color)
                        .child(if title.is_empty() {
                            "Untitled".to_string()
                        } else {
                            title
                        }),
                );
            }
            container
        };
        children.push(title_div.into_any_element());

        let mut tag_elements = Vec::new();
        for tag in &tags {
            let tag_for_remove = tag.clone();
            tag_elements.push(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded_md()
                    .bg(rgb(0x45475a))
                    .text_color(rgb(0x89b4fa))
                    .text_xs()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(tag.clone())
                    .child(
                        div()
                            .cursor(gpui::CursorStyle::PointingHand)
                            .text_color(rgb(0x6c7086))
                            .child("x")
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, _ev, _window, cx| {
                                    this.remove_tag(&tag_for_remove);
                                    cx.notify();
                                }),
                            ),
                    )
                    .into_any_element(),
            );
        }

        let is_tag_zone = self.edit_zone == EditZone::TagInput;
        if is_tag_zone {
            let input_text = self.tag_input.clone();
            let tc = self.tag_input_cursor;
            let (before, after) = split_at_char_col(&input_text, tc);
            children.push(
                div()
                    .w_full()
                    .py(px(4.0))
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0x585b70))
                            .bg(rgb(0x1e1e2e))
                            .text_xs()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(div().text_color(rgb(0xcdd6f4)).child(if before.is_empty() {
                                SharedString::from("")
                            } else {
                                SharedString::from(before.clone())
                            }))
                            .child(
                                div()
                                    .w(px(2.0))
                                    .h(px(14.0))
                                    .bg(rgb(0xcdd6f4))
                                    .flex_shrink_0(),
                            )
                            .child(div().text_color(rgb(0xcdd6f4)).child(if after.is_empty() {
                                if before.is_empty() {
                                    SharedString::from("Type #tag ...")
                                } else {
                                    SharedString::from("")
                                }
                            } else {
                                SharedString::from(after)
                            })),
                    )
                    .into_any_element(),
            );
        } else {
            children.push(
                div()
                    .w_full()
                    .py(px(4.0))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(4.0))
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _ev, _window, cx| {
                            this.edit_zone = EditZone::TagInput;
                            this.populate_tag_input();
                            cx.notify();
                        }),
                    )
                    .children(tag_elements)
                    .into_any_element(),
            );
        }

        children.push(
            div()
                .w_full()
                .py(px(4.0))
                .flex()
                .flex_row()
                .gap(px(16.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x6c7086))
                        .child(format!("Created: {created}")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x6c7086))
                        .child(format!("Updated: {updated}")),
                )
                .into_any_element(),
        );

        children.push(
            div()
                .w_full()
                .h(px(1.0))
                .bg(rgb(0x313244))
                .my(px(4.0))
                .into_any_element(),
        );

        children
    }

    fn render_highlighted_line(
        &self,
        line_idx: usize,
        line_text: &str,
        display_text: String,
        mut line_div: gpui::Div,
        cursor_line: usize,
        cursor_col: usize,
    ) -> gpui::Div {
        let mut highlighted = self
            .cached_highlights
            .get(line_idx)
            .cloned()
            .unwrap_or_else(|| HighlightedLine {
                highlights: vec![],
                image_urls: Vec::new(),
                line_height: 22.0,
                heading_level: None,
                line_bg: None,
            });
        let lh = highlighted.line_height;
        let image_urls = highlighted.image_urls.clone();

        // Table header bold
        if line_text.starts_with('|')
            && line_text
                .chars()
                .all(|c| c == '|' || c == '-' || c == ':' || c == ' ' || c == '\t')
            && !line_text.contains('-')
        {
            let next_is_sep = self
                .cached_lines
                .get(line_idx + 1)
                .map(|l| {
                    l.starts_with('|')
                        && l.chars()
                            .all(|c| c == '|' || c == '-' || c == ':' || c == ' ' || c == '\t')
                        && l.contains('-')
                })
                .unwrap_or(false);
            if next_is_sep {
                highlighted.highlights.insert(
                    0,
                    (
                        0..line_text.len().max(1),
                        HighlightStyle {
                            font_weight: Some(FontWeight::BOLD),
                            ..Default::default()
                        },
                    ),
                );
            }
        }

        line_div =
            line_div
                .h(px(lh))
                .when_some(highlighted.heading_level, |el, level| match level {
                    1 => el.text_2xl(),
                    2 => el.text_xl(),
                    3 => el.text_lg(),
                    4 => el.text_base(),
                    _ => el.text_sm(),
                });

        // Apply line-level background (e.g. code block bg extending to right edge)
        if let Some(bg) = highlighted.line_bg {
            line_div = line_div.bg(bg);
        }

        // Merge selection background using deterministic overlay
        let sel_byte_range = self.selection.as_ref().and_then(|sel| {
            let (sl, sc) = self.byte_to_line_col(sel.start);
            let (el, ec) = self.byte_to_line_col(sel.end);
            if line_idx >= sl && line_idx <= el {
                let from = if line_idx == sl { sc } else { 0 };
                let to = if line_idx == el {
                    ec
                } else {
                    line_text.chars().count()
                };
                if from < to {
                    return Some(
                        char_idx_to_byte(line_text, from)..char_idx_to_byte(line_text, to),
                    );
                }
            }
            None
        });
        if let Some(sel_range) = sel_byte_range {
            let sel_bg: gpui::Hsla = gpui::rgba(0x45475a88).into();
            highlighted.highlights = overlay_selection(highlighted.highlights, sel_range, sel_bg);
        }

        // Render with or without cursor
        if line_idx == cursor_line && self.edit_zone == EditZone::Content {
            let (before, after) = split_at_char_col(&display_text, cursor_col);
            let before_len = before.len();
            let display_len = display_text.len();
            let before_styled = StyledText::new(SharedString::from(before)).with_highlights(
                adjust_highlight_offsets(&highlighted.highlights, 0, before_len),
            );
            let after_styled = if after.is_empty() {
                StyledText::new(SharedString::from(" ")).with_highlights(vec![])
            } else {
                StyledText::new(SharedString::from(after)).with_highlights(
                    adjust_highlight_offsets(&highlighted.highlights, before_len, display_len),
                )
            };
            line_div = line_div
                .child(before_styled)
                .child(
                    div()
                        .w(px(2.0))
                        .h(px(lh - 4.0))
                        .bg(rgb(0xcdd6f4))
                        .flex_shrink_0(),
                )
                .child(after_styled);
        } else {
            line_div = line_div.child(
                StyledText::new(SharedString::from(display_text))
                    .with_highlights(highlighted.highlights),
            );
        }

        for url in &image_urls {
            let resolved = resolve_image_path(self.file_path.as_deref(), url);
            if resolved.exists() {
                line_div = line_div.child(
                    div().ml(px(16.0)).py(px(4.0)).child(
                        img(SharedString::from(resolved.to_string_lossy().to_string()))
                            .object_fit(gpui::ObjectFit::Contain)
                            .max_h(px(200.0)),
                    ),
                );
            } else {
                line_div = line_div.child(
                    div()
                        .ml(px(16.0))
                        .py(px(4.0))
                        .px(px(8.0))
                        .rounded_md()
                        .bg(rgb(0x313244))
                        .text_xs()
                        .text_color(rgb(0x6c7086))
                        .child(format!("[image not found: {url}]")),
                );
            }
        }

        line_div
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = if self.edit_zone == EditZone::Title {
            self.frontmatter
                .as_ref()
                .map(|f| f.title.as_str())
                .unwrap_or("")
        } else if self.edit_zone == EditZone::TagInput {
            &self.tag_input
        } else {
            &self.cached_text
        };
        let byte_start = utf16_to_byte(text, range.start);
        let byte_end = utf16_to_byte(text, range.end);
        if byte_start <= byte_end && byte_end <= text.len() {
            Some(text[byte_start..byte_end].to_string())
        } else {
            None
        }
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.edit_zone == EditZone::Title {
            let text = self
                .frontmatter
                .as_ref()
                .map(|f| f.title.as_str())
                .unwrap_or("");
            let byte_pos = char_idx_to_byte(text, self.title_cursor);
            let utf16_pos = byte_to_utf16(text, byte_pos);
            return Some(UTF16Selection {
                range: utf16_pos..utf16_pos,
                reversed: false,
            });
        }
        if self.edit_zone == EditZone::TagInput {
            let text = &self.tag_input;
            let byte_pos = char_idx_to_byte(text, self.tag_input_cursor);
            let utf16_pos = byte_to_utf16(text, byte_pos);
            return Some(UTF16Selection {
                range: utf16_pos..utf16_pos,
                reversed: false,
            });
        }
        let text = &self.cached_text;
        let range = self
            .selection
            .clone()
            .unwrap_or(self.cursor_pos..self.cursor_pos);
        let start = byte_to_utf16(text, range.start);
        let end = byte_to_utf16(text, range.end);
        Some(UTF16Selection {
            range: start..end,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.ime_state.marked_range.as_ref().map(|r| {
            let text = &self.cached_text;
            byte_to_utf16(text, r.start)..byte_to_utf16(text, r.end)
        })
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.ime_state.clear();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit_zone == EditZone::Title {
            if let Some(fm) = &mut self.frontmatter {
                let byte_pos = char_idx_to_byte(&fm.title, self.title_cursor);
                fm.title.insert_str(byte_pos, new_text);
                self.title_cursor += new_text.chars().count();
                self.dirty = true;
            }
            cx.notify();
            return;
        }
        if self.edit_zone == EditZone::TagInput {
            let byte_pos = char_idx_to_byte(&self.tag_input, self.tag_input_cursor);
            self.tag_input.insert_str(byte_pos, new_text);
            self.tag_input_cursor += new_text.chars().count();
            cx.notify();
            return;
        }
        // Determine byte range to replace
        let byte_range = match range {
            Some(r) => {
                self.selection = None;
                let text = &self.cached_text;
                utf16_to_byte(text, r.start)..utf16_to_byte(text, r.end)
            }
            None => self
                .selection
                .take()
                .unwrap_or(self.cursor_pos..self.cursor_pos),
        };
        self.buffer.edit(byte_range.start, byte_range.end, new_text);
        self.cache_edit(byte_range.start, byte_range.end, new_text);
        self.cursor_pos = byte_range.start + new_text.len();
        self.ime_state.clear();
        self.dirty = true;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edit_zone == EditZone::Title {
            if let Some(fm) = &mut self.frontmatter {
                let byte_pos = char_idx_to_byte(&fm.title, self.title_cursor);
                fm.title.insert_str(byte_pos, new_text);
                self.title_cursor += new_text.chars().count();
                self.dirty = true;
            }
            cx.notify();
            return;
        }
        if self.edit_zone == EditZone::TagInput {
            let byte_pos = char_idx_to_byte(&self.tag_input, self.tag_input_cursor);
            self.tag_input.insert_str(byte_pos, new_text);
            self.tag_input_cursor += new_text.chars().count();
            cx.notify();
            return;
        }
        // Compute byte_range, replacing selection if range is None
        let byte_range = match range {
            Some(r) => {
                self.selection = None;
                let text = &self.cached_text;
                utf16_to_byte(text, r.start)..utf16_to_byte(text, r.end)
            }
            None => self
                .selection
                .take()
                .unwrap_or(self.cursor_pos..self.cursor_pos),
        };

        // Remove existing marked text if present
        if let Some(marked) = self.ime_state.marked_range.clone() {
            self.buffer.delete(marked.start, marked.end);
            self.cache_edit(marked.start, marked.end, "");
            if self.cursor_pos > marked.end {
                self.cursor_pos -= marked.end - marked.start;
            } else if self.cursor_pos > marked.start {
                self.cursor_pos = marked.start;
            }
        }
        self.buffer.insert(byte_range.start, new_text);
        self.cache_edit(byte_range.start, byte_range.start, new_text);
        self.ime_state
            .set_marked(byte_range.start..byte_range.start + new_text.len());
        self.cursor_pos = byte_range.start + new_text.len();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
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
            self.cached_highlights = build_highlights(lines, &self.resolved_colors);
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
                .h(px(22.0))
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
                        let click_col =
                            pixel_to_col(&line_text, event.position.x, ascii_char_width);
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
                        let move_col = pixel_to_col(&line_text, event.position.x, ascii_char_width);
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
                                .bg(rgb(0xcdd6f4))
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

        // Schedule highlight build for next frame if not yet done
        if !has_highlights && !lines.is_empty() {
            self.highlights_dirty = true;
            cx.notify();
        }

        // Register input handler when focused
        if self.focus_handle.is_focused(window) {
            window.handle_input(
                &self.focus_handle,
                ElementInputHandler::new(Bounds::default(), cx.entity()),
                cx,
            );
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .text_color(rgb(0xcdd6f4))
            .text_sm()
            .font_family("monospace")
            .p(px(8.0))
            .children(header_children)
            .children(children)
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

// --- Frontmatter helpers ---

fn parse_frontmatter_gui(raw: &str) -> Option<(Option<Frontmatter>, String)> {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = &trimmed[3..];
    let end_idx = rest.find("---")?;
    let yaml_str = &rest[..end_idx];
    let body = rest[end_idx + 3..].trim_start().to_string();
    let frontmatter: Frontmatter = serde_yaml::from_str(yaml_str).ok()?;
    Some((Some(frontmatter), body))
}

fn format_note_file(frontmatter: &Frontmatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();
    format!("---\n{yaml}---\n{body}")
}

// --- Highlight builder (Tree-sitter for code blocks) ---

/// Build per-line highlights, using Tree-sitter for fenced code blocks.
/// Style a math block delimiter line ($$).
fn math_delim_line(line: &str, math_fg: gpui::Hsla) -> HighlightedLine {
    let dollar_count = line.bytes().take_while(|&b| b == b'$').count();
    let mut highlights = vec![(
        0..dollar_count,
        HighlightStyle {
            color: Some(math_fg),
            fade_out: Some(0.4),
            ..Default::default()
        },
    )];
    if dollar_count < line.len() {
        highlights.push((
            dollar_count..line.len(),
            HighlightStyle {
                color: Some(math_fg),
                ..Default::default()
            },
        ));
    }
    HighlightedLine {
        highlights,
        image_urls: Vec::new(),
        line_height: 22.0,
        heading_level: None,
        line_bg: None,
    }
}

fn build_highlights(lines: &[String], colors: &ResolvedColors) -> Vec<HighlightedLine> {
    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];

        if line.starts_with("```") {
            let lang = line.trim_start_matches('`').trim().to_string();
            let lang_key = zelkova_highlight::resolve_language(&lang);
            result.push(highlight_fence_line(line, colors));
            i += 1;

            let mut code_lines = Vec::new();
            while i < lines.len() && !lines[i].starts_with("```") {
                code_lines.push(lines[i].as_str());
                i += 1;
            }

            if let Some(lang_name) = lang_key {
                let code_text = code_lines.join("\n");
                let styled = zelkova_highlight::highlight_code(&code_text, lang_name);

                let mut line_byte_start = 0usize;
                for &code_line in &code_lines {
                    let line_byte_end = line_byte_start + code_line.len();

                    let mut syntax_hl = Vec::new();
                    for sr in &styled {
                        if sr.range.end <= line_byte_start || sr.range.start >= line_byte_end {
                            continue;
                        }
                        let clamp_start = sr.range.start.max(line_byte_start) - line_byte_start;
                        let clamp_end = sr.range.end.min(line_byte_end) - line_byte_start;
                        if let Some(color) = colors.syntax_color(sr.highlight_index) {
                            syntax_hl.push((
                                clamp_start..clamp_end,
                                HighlightStyle {
                                    color: Some(color),
                                    ..Default::default()
                                },
                            ));
                        }
                    }

                    result.push(HighlightedLine {
                        highlights: syntax_hl,
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: Some(colors.code_bg),
                    });
                    line_byte_start = line_byte_end + 1;
                }
            } else {
                for &code_line in &code_lines {
                    result.push(HighlightedLine {
                        highlights: vec![(
                            0..code_line.len().max(1),
                            HighlightStyle {
                                color: Some(colors.code_fg),
                                ..Default::default()
                            },
                        )],
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: Some(colors.code_bg),
                    });
                }
            }

            if i < lines.len() {
                result.push(highlight_fence_line(&lines[i], colors));
                i += 1;
            }
        } else if line.trim_start().starts_with("$$") {
            let math_fg = colors.math_fg;
            result.push(math_delim_line(line, math_fg));
            i += 1;

            while i < lines.len() {
                let ml = lines[i].len();
                if lines[i].trim_end().ends_with("$$") && lines[i].trim() != "$$" {
                    result.push(HighlightedLine {
                        highlights: vec![(
                            0..ml.max(1),
                            HighlightStyle {
                                color: Some(math_fg),
                                ..Default::default()
                            },
                        )],
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: None,
                    });
                    i += 1;
                    break;
                } else if lines[i].trim() == "$$" {
                    result.push(math_delim_line(&lines[i], math_fg));
                    i += 1;
                    break;
                } else {
                    result.push(HighlightedLine {
                        highlights: vec![(
                            0..ml.max(1),
                            HighlightStyle {
                                color: Some(math_fg),
                                ..Default::default()
                            },
                        )],
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: None,
                    });
                    i += 1;
                }
            }
        } else {
            let context = detect_line_context(line, false);
            result.push(highlight_line(line, &context, colors));
            i += 1;
        }
    }

    result
}

/// Deterministically overlay selection background onto existing highlights.
/// Unlike `combine_highlights`, this always lets selection background win.
fn overlay_selection(
    highlights: Vec<(std::ops::Range<usize>, HighlightStyle)>,
    sel: std::ops::Range<usize>,
    sel_bg: gpui::Hsla,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    if sel.is_empty() {
        return highlights;
    }

    let mut result = Vec::new();
    let mut pos = sel.start;

    for (range, style) in highlights {
        if range.end <= sel.start || range.start >= sel.end {
            result.push((range, style));
            continue;
        }

        // Fill gap in selection before this highlight
        if pos < range.start {
            let gap_end = range.start.min(sel.end);
            if pos < gap_end {
                result.push((
                    pos..gap_end,
                    HighlightStyle {
                        background_color: Some(sel_bg),
                        ..Default::default()
                    },
                ));
            }
        }

        // Part before selection
        if range.start < sel.start {
            result.push((range.start..sel.start, style.clone()));
        }

        // Overlap — override background with selection
        let o_start = range.start.max(sel.start);
        let o_end = range.end.min(sel.end);
        let mut merged = style.clone();
        merged.background_color = Some(sel_bg);
        result.push((o_start..o_end, merged));
        pos = o_end;

        // Part after selection
        if range.end > sel.end {
            result.push((sel.end..range.end, style));
        }
    }

    // Fill remaining gap at end of selection
    if pos < sel.end {
        result.push((
            pos..sel.end,
            HighlightStyle {
                background_color: Some(sel_bg),
                ..Default::default()
            },
        ));
    }

    result.sort_by_key(|(r, _)| r.start);
    result
}

// --- Utility functions ---

fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    if text.ends_with('\n') {
        lines.push(String::new());
    }
    lines
}

fn split_at_char_col(s: &str, col: usize) -> (String, String) {
    let mut char_count = 0;
    for (byte_idx, _) in s.char_indices() {
        if char_count == col {
            return (s[..byte_idx].to_string(), s[byte_idx..].to_string());
        }
        char_count += 1;
    }
    (s.to_string(), String::new())
}

fn adjust_highlight_offsets(
    highlights: &[(std::ops::Range<usize>, gpui::HighlightStyle)],
    offset_start: usize,
    offset_end: usize,
) -> Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> {
    highlights
        .iter()
        .filter_map(|(range, style)| {
            if range.start >= offset_end || range.end <= offset_start {
                return None;
            }
            let new_start = range.start.max(offset_start) - offset_start;
            let new_end = range.end.min(offset_end) - offset_start;
            Some((new_start..new_end, style.clone()))
        })
        .collect()
}

fn pixel_to_col(line: &str, pixel_x: gpui::Pixels, ascii_w: f32) -> usize {
    let target = f32::from(pixel_x);
    let mut width = 0.0;
    let mut col = 0;
    for c in line.chars() {
        let char_w = if c.is_ascii() {
            ascii_w
        } else if c as u32 > 0x2FFF {
            ascii_w * 2.0
        } else {
            ascii_w
        };
        if width + char_w / 2.0 > target {
            return col;
        }
        width += char_w;
        col += 1;
    }
    col
}

fn byte_to_utf16(text: &str, byte_pos: usize) -> usize {
    text[..byte_pos]
        .chars()
        .map(|c| if c as u32 > 0xFFFF { 2 } else { 1 })
        .sum()
}

fn utf16_to_byte(text: &str, utf16_pos: usize) -> usize {
    let mut count = 0;
    for (i, c) in text.char_indices() {
        if count >= utf16_pos {
            return i;
        }
        count += if c as u32 > 0xFFFF { 2 } else { 1 };
    }
    text.len()
}

fn char_idx_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Parse `#xxx` tokens from tag input text.
/// A valid tag is `#` followed by one or more non-whitespace characters.
fn parse_tags_from_input(input: &str) -> std::collections::HashSet<String> {
    let mut tags = std::collections::HashSet::new();
    for token in input.split_whitespace() {
        if let Some(tag) = token.strip_prefix('#') {
            if !tag.is_empty() {
                tags.insert(tag.to_string());
            }
        }
    }
    tags
}

#[cfg(test)]
mod tag_tests {
    use super::*;

    #[test]
    fn parse_single_tag() {
        let tags = parse_tags_from_input("#work");
        assert!(tags.contains("work"));
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn parse_multiple_tags() {
        let tags = parse_tags_from_input("#work #meeting #project");
        assert_eq!(tags.len(), 3);
        assert!(tags.contains("work"));
        assert!(tags.contains("meeting"));
        assert!(tags.contains("project"));
    }

    #[test]
    fn discard_invalid_tokens() {
        let tags = parse_tags_from_input("#work garbage #meeting");
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("work"));
        assert!(tags.contains("meeting"));
    }

    #[test]
    fn empty_hash_discarded() {
        let tags = parse_tags_from_input("# #valid");
        assert_eq!(tags.len(), 1);
        assert!(tags.contains("valid"));
    }

    #[test]
    fn empty_input() {
        let tags = parse_tags_from_input("");
        assert!(tags.is_empty());
    }

    #[test]
    fn full_width_space_normalization() {
        let input = "#work\u{3000}#meeting\u{3000}garbage";
        let normalized = input.replace('\u{3000}', " ");
        let tags = parse_tags_from_input(&normalized);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("work"));
        assert!(tags.contains("meeting"));
    }

    #[test]
    fn duplicate_tags_deduped() {
        let tags = parse_tags_from_input("#work #work #meeting");
        assert_eq!(tags.len(), 2);
    }
}

/// Resolve an image URL to an absolute path.
/// - Absolute paths are kept as-is.
/// - `~/` is expanded to the home directory.
/// - Relative paths are resolved against the note file's directory.
fn resolve_image_path(note_path: Option<&std::path::Path>, url: &str) -> std::path::PathBuf {
    let url = url.trim();
    if url.starts_with('/') {
        return std::path::PathBuf::from(url);
    }
    if let Some(rest) = url.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
        return std::path::PathBuf::from(format!("/{rest}"));
    }
    if let Some(dir) = note_path.and_then(|p| p.parent()) {
        return dir.join(url);
    }
    std::path::PathBuf::from(url)
}
