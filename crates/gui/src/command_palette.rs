use gpui::{
    App, Context, FocusHandle, Focusable, IntoElement, Render, SharedString, StyledText, Window,
    div, prelude::*, px, rgb,
};

// ---------------------------------------------------------------------------
// Arg type system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ArgType {
    FreeText { default: Option<String> },
    Select { options: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct ArgSpec {
    pub prompt: String,
    pub arg_type: ArgType,
    pub optional: bool,
}

// ---------------------------------------------------------------------------
// Command spec
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub label: String,
    pub args: Vec<ArgSpec>,
}

impl CommandSpec {
    pub fn no_arg(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            args: Vec::new(),
        }
    }

    pub fn with_args(label: impl Into<String>, args: Vec<ArgSpec>) -> Self {
        Self {
            label: label.into(),
            args,
        }
    }
}

// ---------------------------------------------------------------------------
// Palette phase
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    SelectCommand,
    InputArg {
        index: usize,
        values: Vec<Option<String>>,
    },
}

// ---------------------------------------------------------------------------
// CommandPalette
// ---------------------------------------------------------------------------

pub struct CommandPalette {
    phase: Phase,
    commands: Vec<CommandSpec>,

    // SelectCommand state
    query: String,
    query_cursor: usize,
    filtered: Vec<usize>,
    selected: usize,

    // InputArg state
    arg_input: String,
    arg_cursor: usize,
    arg_selected: usize,

    focus_handle: FocusHandle,
}

impl CommandPalette {
    pub fn new(folder_names: &[String], note_titles: &[String], cx: &mut App) -> Self {
        let commands = super::keymap::all_command_specs(folder_names, note_titles);
        let filtered: Vec<usize> = (0..commands.len()).collect();
        Self {
            phase: Phase::SelectCommand,
            commands,
            query: String::new(),
            query_cursor: 0,
            filtered,
            selected: 0,
            arg_input: String::new(),
            arg_cursor: 0,
            arg_selected: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn move_cursor_left(&mut self) {
        match self.phase {
            Phase::SelectCommand => {
                if self.query_cursor > 0 {
                    self.query_cursor -= 1;
                }
            }
            Phase::InputArg { .. } => {
                if self.arg_cursor > 0 {
                    self.arg_cursor -= 1;
                }
            }
        }
    }

    pub fn move_cursor_right(&mut self) {
        match self.phase {
            Phase::SelectCommand => {
                if self.query_cursor < self.query.len() {
                    self.query_cursor += 1;
                }
            }
            Phase::InputArg { .. } => {
                if self.arg_cursor < self.arg_input.len() {
                    self.arg_cursor += 1;
                }
            }
        }
    }

    pub fn handle_backspace(&mut self) {
        match self.phase {
            Phase::SelectCommand => {
                if self.query_cursor > 0 {
                    self.query_cursor -= 1;
                    self.query.remove(self.query_cursor);
                    self.update_filter();
                }
            }
            Phase::InputArg { .. } => {
                if self.arg_cursor > 0 {
                    self.arg_cursor -= 1;
                    self.arg_input.remove(self.arg_cursor);
                    if matches!(self.current_arg_type(), Some(ArgType::Select { .. })) {
                        self.arg_selected = 0;
                    }
                }
            }
        }
    }

    pub fn move_selection_up(&mut self) {
        match self.phase {
            Phase::SelectCommand => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            Phase::InputArg { .. } => {
                if matches!(self.current_arg_type(), Some(ArgType::Select { .. })) {
                    if self.arg_selected > 0 {
                        self.arg_selected -= 1;
                    }
                }
            }
        }
    }

    pub fn move_selection_down(&mut self) {
        match self.phase {
            Phase::SelectCommand => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                }
            }
            Phase::InputArg { .. } => {
                if matches!(self.current_arg_type(), Some(ArgType::Select { .. })) {
                    let count = self.filtered_arg_options().len();
                    if self.arg_selected + 1 < count {
                        self.arg_selected += 1;
                    }
                }
            }
        }
    }

    fn current_arg_type(&self) -> Option<ArgType> {
        if let Phase::InputArg { index, .. } = &self.phase {
            let cmd_idx = self.filtered.get(self.selected)?;
            self.commands[*cmd_idx]
                .args
                .get(*index)
                .map(|s| s.arg_type.clone())
        } else {
            None
        }
    }

    /// Fuzzy-filtered option list for the current Select arg.
    fn filtered_arg_options(&self) -> Vec<String> {
        if let Some(ArgType::Select { options }) = self.current_arg_type() {
            options
                .iter()
                .filter(|opt| fuzzy_match(&self.arg_input, opt))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Handle confirm (Enter key). Returns Some((command_label, arg_values)) when
    /// a command should be executed, None otherwise.
    pub fn handle_confirm(&mut self) -> Option<(String, Vec<Option<String>>)> {
        match &self.phase.clone() {
            Phase::SelectCommand => {
                let &cmd_idx = self.filtered.get(self.selected)?;
                let cmd = &self.commands[cmd_idx];
                if cmd.args.is_empty() {
                    return Some((cmd.label.clone(), Vec::new()));
                }
                self.phase = Phase::InputArg {
                    index: 0,
                    values: Vec::new(),
                };
                self.arg_input = String::new();
                self.arg_cursor = 0;
                self.arg_selected = 0;
                None
            }
            Phase::InputArg { index, values } => {
                let cmd_idx = *self.filtered.get(self.selected)?;
                let i = *index;
                let spec = self.commands[cmd_idx].args.get(i)?.clone();

                let val = match &spec.arg_type {
                    ArgType::Select { .. } => {
                        let filtered = self.filtered_arg_options();
                        let selected = filtered.get(self.arg_selected).cloned();
                        if !spec.optional && selected.is_none() {
                            return None;
                        }
                        selected
                    }
                    ArgType::FreeText { .. } => {
                        if self.arg_input.is_empty() {
                            if spec.optional {
                                None
                            } else {
                                return None;
                            }
                        } else {
                            Some(self.arg_input.clone())
                        }
                    }
                };

                self.advance_arg(cmd_idx, i, values.clone(), val)
            }
        }
    }

    fn advance_arg(
        &mut self,
        cmd_idx: usize,
        current_index: usize,
        mut values: Vec<Option<String>>,
        val: Option<String>,
    ) -> Option<(String, Vec<Option<String>>)> {
        values.push(val);
        let cmd = &self.commands[cmd_idx];
        let next_index = current_index + 1;
        if next_index < cmd.args.len() {
            self.phase = Phase::InputArg {
                index: next_index,
                values,
            };
            self.arg_input = String::new();
            self.arg_cursor = 0;
            self.arg_selected = 0;
            None
        } else {
            Some((cmd.label.clone(), values))
        }
    }

    /// Handle ESC. Returns true if the palette should be closed.
    pub fn handle_back(&mut self) -> bool {
        match &self.phase.clone() {
            Phase::SelectCommand => true,
            Phase::InputArg { index: 0, .. } => {
                self.phase = Phase::SelectCommand;
                self.arg_input.clear();
                self.arg_cursor = 0;
                false
            }
            Phase::InputArg { index, values } => {
                let prev = *index - 1;
                let mut prev_values = values.clone();
                prev_values.pop();
                let prev_val = prev_values.last().and_then(|v| v.clone());
                self.phase = Phase::InputArg {
                    index: prev,
                    values: prev_values,
                };
                self.arg_input = prev_val.unwrap_or_default();
                self.arg_cursor = self.arg_input.len();
                self.arg_selected = 0;
                false
            }
        }
    }

    fn update_filter(&mut self) {
        self.filtered = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| fuzzy_match(&self.query, &cmd.label))
            .map(|(i, _)| i)
            .collect();
        if self.selected >= self.filtered.len() && !self.filtered.is_empty() {
            self.selected = self.filtered.len() - 1;
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
        let palette_max_height = px(350.0);

        if !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window);
        }

        if self.focus_handle.is_focused(window) {
            window.handle_input(
                &self.focus_handle,
                gpui::ElementInputHandler::new(gpui::Bounds::default(), cx.entity()),
                cx,
            );
        }

        let dim_color: gpui::Hsla = gpui::rgba(0xa6adc8_ff).into();

        let content = match &self.phase {
            Phase::SelectCommand => {
                let prompt = if self.query.is_empty() {
                    let text = "> \u{2588}".to_string();
                    StyledText::new(SharedString::from(text.clone())).with_highlights(vec![
                        (
                            0..2,
                            gpui::HighlightStyle {
                                color: Some(dim_color),
                                ..Default::default()
                            },
                        ),
                        (
                            2..3,
                            gpui::HighlightStyle {
                                color: Some(rgb(0xcdd6f4).into()),
                                ..Default::default()
                            },
                        ),
                    ])
                } else {
                    let before = format!("> {}", &self.query[..self.query_cursor]);
                    let after = &self.query[self.query_cursor..];
                    let text = format!("{}\u{2588}{}", before, after);
                    let cursor_start = before.len();
                    let cursor_end = cursor_start + "\u{2588}".len();
                    StyledText::new(SharedString::from(text)).with_highlights(vec![
                        (
                            0..2,
                            gpui::HighlightStyle {
                                color: Some(dim_color),
                                ..Default::default()
                            },
                        ),
                        (
                            cursor_start..cursor_end,
                            gpui::HighlightStyle {
                                color: Some(rgb(0xcdd6f4).into()),
                                ..Default::default()
                            },
                        ),
                    ])
                };

                let items: Vec<_> = self
                    .filtered
                    .iter()
                    .enumerate()
                    .take(10)
                    .map(|(i, &idx)| {
                        let is_sel = i == self.selected;
                        let bg = if is_sel { rgb(0x45475a) } else { rgb(0x313244) };
                        div()
                            .px_3()
                            .py_1()
                            .bg(bg)
                            .text_color(rgb(0xcdd6f4))
                            .text_sm()
                            .child(self.commands[idx].label.clone())
                    })
                    .collect();

                div()
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(0x585b70))
                            .child(prompt),
                    )
                    .children(items)
            }
            Phase::InputArg { index, values, .. } => {
                let cmd_idx = self.filtered.get(self.selected).copied().unwrap_or(0);
                let cmd = &self.commands[cmd_idx];
                let spec = &cmd.args[*index];

                // Progress indicator
                let step = format!("{} [{}/{}]", cmd.label, index + 1, cmd.args.len());

                let prompt_text = if self.arg_input.is_empty() {
                    let text = format!("{}: \u{2588}", spec.prompt);
                    let placeholder = if spec.optional { "(optional)" } else { "" };
                    let full = format!("{}{}", text, placeholder);
                    let cursor_start = text.len() - "\u{2588}".len();
                    let cursor_end = text.len();
                    StyledText::new(SharedString::from(full.clone())).with_highlights(vec![
                        (
                            0..spec.prompt.len() + 2,
                            gpui::HighlightStyle {
                                color: Some(rgb(0xcdd6f4).into()),
                                ..Default::default()
                            },
                        ),
                        (
                            cursor_start..cursor_end,
                            gpui::HighlightStyle {
                                color: Some(rgb(0xcdd6f4).into()),
                                ..Default::default()
                            },
                        ),
                        (
                            text.len()..full.len(),
                            gpui::HighlightStyle {
                                color: Some(dim_color),
                                ..Default::default()
                            },
                        ),
                    ])
                } else {
                    let prefix = format!("{}: ", spec.prompt);
                    let before = &self.arg_input[..self.arg_cursor];
                    let after = &self.arg_input[self.arg_cursor..];
                    let text = format!("{}\u{2588}{}", before, after);
                    let full = format!("{}{}", prefix, text);
                    let prompt_len = prefix.len();
                    let cursor_start = prompt_len + before.len();
                    let cursor_end = cursor_start + "\u{2588}".len();
                    StyledText::new(SharedString::from(full)).with_highlights(vec![
                        (
                            0..prompt_len,
                            gpui::HighlightStyle {
                                color: Some(dim_color),
                                ..Default::default()
                            },
                        ),
                        (
                            cursor_start..cursor_end,
                            gpui::HighlightStyle {
                                color: Some(rgb(0xcdd6f4).into()),
                                ..Default::default()
                            },
                        ),
                    ])
                };

                match &spec.arg_type {
                    ArgType::FreeText { .. } => div()
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .text_xs()
                                .text_color(rgb(0xa6adc8))
                                .child(step),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .border_b_1()
                                .border_color(rgb(0x585b70))
                                .child(prompt_text),
                        ),
                    ArgType::Select { .. } => {
                        let filtered = self.filtered_arg_options();
                        let items: Vec<_> = filtered
                            .iter()
                            .enumerate()
                            .take(10)
                            .map(|(i, opt)| {
                                let is_sel = i == self.arg_selected;
                                let bg = if is_sel { rgb(0x45475a) } else { rgb(0x313244) };
                                div()
                                    .px_3()
                                    .py_1()
                                    .bg(bg)
                                    .text_color(rgb(0xcdd6f4))
                                    .text_sm()
                                    .child(opt.clone())
                            })
                            .collect();

                        div()
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .text_xs()
                                    .text_color(rgb(0xa6adc8))
                                    .child(step),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .border_b_1()
                                    .border_color(rgb(0x585b70))
                                    .child(prompt_text),
                            )
                            .children(items)
                    }
                }
            }
        };

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
            .child(content)
    }
}

// ---------------------------------------------------------------------------
// EntityInputHandler
// ---------------------------------------------------------------------------

impl gpui::EntityInputHandler for CommandPalette {
    fn text_for_range(
        &mut self,
        range: std::ops::Range<usize>,
        _adjusted_range: &mut Option<std::ops::Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = match self.phase {
            Phase::SelectCommand => &self.query,
            Phase::InputArg { .. } => &self.arg_input,
        };
        let start = range.start.min(text.len());
        let end = range.end.min(text.len());
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::UTF16Selection> {
        let (len, cursor) = match self.phase {
            Phase::SelectCommand => (self.query.len(), self.query_cursor),
            Phase::InputArg { .. } => (self.arg_input.len(), self.arg_cursor),
        };
        Some(gpui::UTF16Selection {
            range: cursor..cursor,
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
        range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.phase {
            Phase::SelectCommand => {
                replace_in_string(&mut self.query, &mut self.query_cursor, range, new_text);
                self.update_filter();
            }
            Phase::InputArg { .. } => {
                replace_in_string(&mut self.arg_input, &mut self.arg_cursor, range, new_text);
                if matches!(self.current_arg_type(), Some(ArgType::Select { .. })) {
                    self.arg_selected = 0;
                }
            }
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<std::ops::Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range, new_text, _window, cx);
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn replace_in_string(
    s: &mut String,
    cursor: &mut usize,
    range: Option<std::ops::Range<usize>>,
    new_text: &str,
) {
    match range {
        Some(r) => {
            let start = r.start.min(s.len());
            let end = r.end.min(s.len());
            s.replace_range(start..end, new_text);
            *cursor = start + new_text.len();
        }
        None => {
            let pos = (*cursor).min(s.len());
            s.insert_str(pos, new_text);
            *cursor = pos + new_text.len();
        }
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
        assert!(fuzzy_match("cn", "Create Note"));
        assert!(fuzzy_match("cf", "Create Folder"));
        assert!(fuzzy_match("", "anything"));
        assert!(!fuzzy_match("xyz", "Create Note"));
    }

    #[test]
    fn replace_in_string_insert() {
        let mut s = "hello".to_string();
        let mut cursor = 5;
        replace_in_string(&mut s, &mut cursor, None, " world");
        assert_eq!(s, "hello world");
        assert_eq!(cursor, 11);
    }

    #[test]
    fn replace_in_string_delete() {
        let mut s = "hello".to_string();
        let mut cursor = 5;
        replace_in_string(&mut s, &mut cursor, Some(4..5), "");
        assert_eq!(s, "hell");
        assert_eq!(cursor, 4);
    }

    #[test]
    fn replace_in_string_backspace() {
        let mut s = "abc".to_string();
        let mut cursor = 3;
        replace_in_string(&mut s, &mut cursor, Some(2..3), "");
        assert_eq!(s, "ab");
        assert_eq!(cursor, 2);
    }
}
