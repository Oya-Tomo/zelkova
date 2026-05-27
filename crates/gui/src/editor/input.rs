use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use super::{EditZone, Editor};
use crate::editor::util::{byte_to_utf16, char_idx_to_byte, utf16_to_byte};

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
