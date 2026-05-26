use std::path::PathBuf;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription, Window, div,
    prelude::*, px,
};
use zelkova_config::EditorColors;

use crate::editor::Editor;
use crate::editor::parse_hex;
use crate::preview::Preview;
use crate::{NextPane, PrevPane, ToggleViewMode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Editor,
    Split,
    Preview,
}

pub struct Tab {
    pub title: String,
    pub file_path: Option<PathBuf>,
    pub editor: Entity<Editor>,
    pub preview: Entity<Preview>,
    pub view_mode: ViewMode,
}

pub struct PaneManager {
    tabs: Vec<Tab>,
    active_tab: usize,
    focus_handle: FocusHandle,
    theme: EditorColors,
    socket_path: Option<PathBuf>,
    _editor_subscriptions: Vec<Subscription>,
}

impl PaneManager {
    pub fn new(cx: &mut App) -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            focus_handle: cx.focus_handle(),
            theme: EditorColors::default(),
            socket_path: None,
            _editor_subscriptions: Vec::new(),
        }
    }

    pub fn set_socket_path(&mut self, path: PathBuf) {
        self.socket_path = Some(path);
    }

    pub fn set_theme(&mut self, theme: EditorColors) {
        self.theme = theme;
    }

    pub fn open_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("file_stem is valid because PathBuf came from a valid file path")
            .to_string();

        // Check if already open
        for (i, tab) in self.tabs.iter().enumerate() {
            if tab.file_path.as_ref() == Some(&path) {
                self.active_tab = i;
                cx.notify();
                return;
            }
        }

        let editor = cx.new(|cx| match Editor::load(path.clone(), cx) {
            Ok(e) => e,
            Err(_) => Editor::new(cx),
        });
        // Pass socket path to editor for save notifications
        if let Some(ref socket) = self.socket_path {
            editor.update(cx, |ed, _| ed.set_socket_path(socket.clone()));
        }
        let text = editor.read(cx).text().to_string();
        let preview = cx.new(|cx| Preview::from_markdown(&text, cx));

        self.tabs.push(Tab {
            title,
            file_path: Some(path),
            editor: editor.clone(),
            preview,
            view_mode: ViewMode::Editor,
        });
        self.active_tab = self.tabs.len() - 1;

        // Observe editor for title changes and preview sync
        let sub = cx.observe(&editor, |this, editor, cx| {
            let active = this.active_tab;
            if let Some(tab) = this.tabs.get_mut(active) {
                if tab.editor == editor {
                    let new_title = editor.read(cx).title().to_string();
                    if tab.title != new_title {
                        tab.title = new_title;
                    }
                    let text = editor.read(cx).text().to_string();
                    tab.preview.update(cx, |p, _| p.update_content(&text));
                    cx.notify();
                }
            }
        });
        self._editor_subscriptions.push(sub);

        cx.notify();
    }

    pub fn close_active_tab(&mut self) {
        if self.tabs.is_empty() {
            return;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    pub fn active_editor(&self) -> Option<&Entity<Editor>> {
        self.tabs.get(self.active_tab).map(|t| &t.editor)
    }

    pub fn active_tab_info(&self) -> (Option<PathBuf>, Option<String>) {
        self.tabs
            .get(self.active_tab)
            .map(|t| (t.file_path.clone(), Some(t.title.clone())))
            .unwrap_or((None, None))
    }

    pub fn active_editor_title(&self, cx: &App) -> (Option<PathBuf>, Option<String>) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            let path = tab.file_path.clone();
            let title = tab.editor.read(cx).title().to_string();
            return (path, Some(title));
        }
        (None, None)
    }

    pub fn handle_next_pane(
        &mut self,
        _: &NextPane,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
    }

    pub fn handle_prev_pane(
        &mut self,
        _: &PrevPane,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if self.tabs.is_empty() {
            return;
        }
        self.active_tab = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
    }

    pub fn handle_toggle_view(
        &mut self,
        _: &ToggleViewMode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.view_mode = match tab.view_mode {
                ViewMode::Editor => ViewMode::Split,
                ViewMode::Split => ViewMode::Preview,
                ViewMode::Preview => ViewMode::Editor,
            };
            // Sync preview content when switching away from editor-only mode
            let text = tab.editor.read(cx).text().to_string();
            tab.preview.update(cx, |p, _| p.update_content(&text));
        }
    }
}

impl Focusable for PaneManager {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PaneManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = parse_hex("#1e1e2e");
        let border = parse_hex("#313244");
        let text = parse_hex("#cdd6f4");
        let text_dim = parse_hex("#a6adc8");
        let tab_bar_bg = parse_hex("#181825");

        // Tab bar
        let tab_bar = div()
            .flex()
            .flex_row()
            .w_full()
            .h(px(32.0))
            .bg(tab_bar_bg)
            .border_b_1()
            .border_color(border)
            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                let is_active = i == self.active_tab;
                let tab_bg = if is_active { bg } else { tab_bar_bg };
                let tab_text = if is_active { text } else { text_dim };
                div()
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .bg(tab_bg)
                    .border_r_1()
                    .border_color(border)
                    .text_color(tab_text)
                    .text_xs()
                    .child(tab.title.clone())
            }));

        // Content area
        let content = if let Some(tab) = self.tabs.get(self.active_tab) {
            match tab.view_mode {
                ViewMode::Editor => div().flex_1().child(tab.editor.clone()).into_any_element(),
                ViewMode::Preview => div().flex_1().child(tab.preview.clone()).into_any_element(),
                ViewMode::Split => div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(div().flex_1().child(tab.editor.clone()))
                    .child(div().w(px(1.0)).bg(border))
                    .child(div().flex_1().child(tab.preview.clone()))
                    .into_any_element(),
            }
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(text_dim)
                .text_sm()
                .child("No open tabs")
                .into_any_element()
        };

        // Focus the active view, or self when no tabs are open
        if let Some(tab) = self.tabs.get(self.active_tab) {
            match tab.view_mode {
                ViewMode::Editor => tab.editor.focus_handle(cx).focus(window),
                ViewMode::Preview => tab.preview.focus_handle(cx).focus(window),
                ViewMode::Split => tab.editor.focus_handle(cx).focus(window),
            }
        } else {
            self.focus_handle.focus(window);
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle)
            .child(tab_bar)
            .child(content)
            .on_action(cx.listener(PaneManager::handle_next_pane))
            .on_action(cx.listener(PaneManager::handle_prev_pane))
            .on_action(cx.listener(PaneManager::handle_toggle_view))
    }
}
