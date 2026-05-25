use gpui::{
    div, px, rgb,
    prelude::*,
    App, Context, FocusHandle, Focusable, IntoElement, Render, SharedString, StyledText, Window,
};

pub struct CommandPalette {
    query: String,
    entries: Vec<(String, String)>,
    filtered: Vec<usize>,
    selected: usize,
    focus_handle: FocusHandle,
}

impl CommandPalette {
    pub fn new(cx: &mut App) -> Self {
        let entries = super::keymap::all_action_entries();
        let filtered: Vec<usize> = (0..entries.len()).collect();
        Self {
            query: String::new(),
            entries,
            filtered,
            selected: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_selection_down(&mut self) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
        }
    }

    pub fn selected_action(&self) -> Option<&str> {
        self.filtered
            .get(self.selected)
            .map(|&i| self.entries[i].0.as_str())
    }

    fn update_filter(&mut self) {
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, (_, label))| fuzzy_match(&self.query, label))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() {
            self.selected = 0;
        }
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette_width = px(500.0);
        let palette_max_height = px(300.0);

        // Auto-focus on first render
        if !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window);
        }

        // Register input handler when focused
        if self.focus_handle.is_focused(window) {
            window.handle_input(
                &self.focus_handle,
                gpui::ElementInputHandler::new(gpui::Bounds::default(), cx.entity()),
                cx,
            );
        }

        let dim_color: gpui::Hsla = gpui::rgba(0xa6adc8_ff).into();
        let query_display = if self.query.is_empty() {
            let text = "> Type to search actions...";
            StyledText::new(SharedString::from(text))
                .with_highlights(vec![(0..text.len(), gpui::HighlightStyle {
                    color: Some(dim_color),
                    ..Default::default()
                })])
        } else {
            let text = format!("> {}", self.query);
            let prompt_len = 2; // "> "
            StyledText::new(SharedString::from(text.clone()))
                .with_highlights(vec![(0..prompt_len, gpui::HighlightStyle {
                    color: Some(dim_color),
                    ..Default::default()
                })])
        };

        let visible_items: Vec<_> = self
            .filtered
            .iter()
            .enumerate()
            .take(10)
            .map(|(i, &idx)| {
                let (_, label) = &self.entries[idx];
                let is_selected = i == self.selected;
                let bg = if is_selected { rgb(0x45475a) } else { rgb(0x313244) };
                div()
                    .px_3()
                    .py_1()
                    .bg(bg)
                    .text_color(rgb(0xcdd6f4))
                    .text_sm()
                    .child(label.clone())
            })
            .collect();

        div()
            .absolute()
            .top(px(200.0))
            .left(px(250.0))
            .w(palette_width)
            .max_h(palette_max_height)
            .bg(rgb(0x313244))
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x585b70))
            .shadow_lg()
            .flex()
            .flex_col()
            .text_color(rgb(0xcdd6f4))
            .track_focus(&self.focus_handle)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0x585b70))
                    .child(query_display),
            )
            .children(visible_items)
    }
}

impl gpui::EntityInputHandler for CommandPalette {
    fn text_for_range(
        &mut self,
        range: std::ops::Range<usize>,
        _adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.query.clone();
        let byte_start = range.start.min(text.len());
        let byte_end = range.end.min(text.len());
        Some(text[byte_start..byte_end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::UTF16Selection> {
        let len = self.query.len();
        Some(gpui::UTF16Selection {
            range: len..len,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<std::ops::Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.query.push_str(new_text);
        self.update_filter();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.query.push_str(new_text);
        self.update_filter();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: std::ops::Range<usize>,
        _element_bounds: gpui::Bounds<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::Bounds<gpui::Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<gpui::Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

fn fuzzy_match(query: &str, target: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query_lower = query.to_lowercase();
    let target_lower = target.to_lowercase();
    let mut query_chars = query_lower.chars().peekable();
    for c in target_lower.chars() {
        if c == *query_chars.peek().unwrap_or(&'\0') {
            query_chars.next();
        }
        if query_chars.peek().is_none() {
            return true;
        }
    }
    query_chars.peek().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match("sn", "Search Notes"));
        assert!(fuzzy_match("cp", "Open Command Palette"));
        assert!(fuzzy_match("", "anything"));
        assert!(!fuzzy_match("xyz", "Search Notes"));
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("sn", "SEARCH NOTES"));
        assert!(fuzzy_match("SN", "search notes"));
    }
}
