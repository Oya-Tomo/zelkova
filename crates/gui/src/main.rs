mod command_palette;
mod editor;
mod keymap;
mod pane;
mod preview;

use std::path::PathBuf;

use gpui::{
    App, Application, Bounds, Context, Entity, SharedString, Window, WindowBounds, WindowOptions,
    actions, div, prelude::*, px, size,
};
use zelkova_config::AppConfig;

actions!(
    zelkova,
    [
        OpenCommandPalette,
        SearchNotes,
        CreateNote,
        ListNotes,
        ShowTags,
        ToggleSidebar,
        SaveNote,
        Quit,
        MoveUp,
        MoveDown,
        MoveLeft,
        MoveRight,
        Backspace,
        InsertNewline,
        NextPane,
        PrevPane,
        ToggleViewMode,
        Undo,
        Redo,
        Confirm,
        Cancel,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
    ]
);

struct ZelkovaApp {
    notes: Vec<NoteEntry>,
    selected: Option<usize>,
    sidebar_visible: bool,
    command_palette: Option<Entity<command_palette::CommandPalette>>,
    pane_manager: Entity<pane::PaneManager>,
    config: AppConfig,
    ui_colors: zelkova_config::UiColors,
}

struct NoteEntry {
    id: String,
    title: String,
    path: PathBuf,
}

impl ZelkovaApp {
    fn new(config: AppConfig, cx: &mut App) -> Self {
        let mut notes = Vec::new();

        if config.daemon.socket_path.exists() {
            let client = zelkova_rpc::client::RpcClient::new(&config.daemon.socket_path);
            if let Ok(result) = client.list_notes(None) {
                notes = result
                    .notes
                    .into_iter()
                    .map(|n| NoteEntry {
                        id: n.id.to_string(),
                        title: n.title,
                        path: n.path,
                    })
                    .collect();
            }
        }

        let theme = zelkova_config::ThemeConfig::load().unwrap_or_default();
        let ui_colors = theme.ui.clone();
        let editor_colors = theme.editor.clone();

        let pane_manager = cx.new(|cx| {
            let mut pm = pane::PaneManager::new(cx);
            pm.set_socket_path(config.daemon.socket_path.clone());
            pm.set_theme(editor_colors);
            pm
        });

        Self {
            notes,
            selected: None,
            sidebar_visible: true,
            command_palette: None,
            pane_manager,
            config,
            ui_colors,
        }
    }

    fn handle_open_command_palette(
        &mut self,
        _: &OpenCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.is_none() {
            let palette = cx.new(|cx| command_palette::CommandPalette::new(cx));
            palette.update(cx, |_, cx| cx.focus_handle()).focus(window);
            self.command_palette = Some(palette);
            cx.notify();
        }
    }

    fn handle_toggle_sidebar(
        &mut self,
        _: &ToggleSidebar,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    fn handle_quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn handle_cancel(&mut self, _: &Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        self.command_palette = None;
        cx.notify();
    }

    fn handle_save(&mut self, _: &SaveNote, _window: &mut Window, cx: &mut Context<Self>) {
        // Sync sidebar title from the active editor's frontmatter
        let (path, title) = self.pane_manager.read(cx).active_editor_title(cx);
        if let (Some(path), Some(title)) = (path, title) {
            for note in &mut self.notes {
                if note.path == path {
                    note.title = title;
                    break;
                }
            }
        }
    }

    fn handle_move_up(&mut self, _: &MoveUp, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.move_selection_up());
        }
    }

    fn handle_move_down(&mut self, _: &MoveDown, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.move_selection_down());
        }
    }

    fn handle_confirm(&mut self, _: &Confirm, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(sel) = self.selected {
            if let Some(note) = self.notes.get(sel) {
                let path = note.path.clone();
                self.pane_manager.update(cx, |pm, cx| pm.open_tab(path, cx));
            }
        }
    }

    fn handle_create_note(&mut self, _: &CreateNote, _window: &mut Window, cx: &mut Context<Self>) {
        if self.config.daemon.socket_path.exists() {
            let client = zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
            if let Ok(result) = client.create_note(None, None, Vec::new()) {
                let path = result.path.clone();
                // Add to local list directly instead of re-fetching
                self.notes.push(NoteEntry {
                    id: result.id.to_string(),
                    title: result.title.clone(),
                    path: result.path,
                });
                self.pane_manager.update(cx, |pm, cx| pm.open_tab(path, cx));
                cx.notify();
            }
        }
    }
}

impl Render for ZelkovaApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_manager.clone();
        let ui = &self.ui_colors;
        let bg = editor::parse_hex(&ui.bg);
        let sidebar_bg = editor::parse_hex(&ui.sidebar_bg);
        let border = editor::parse_hex(&ui.border);
        let text = editor::parse_hex(&ui.text);
        let text_dim = editor::parse_hex(&ui.text_dim);
        let selection_bg = editor::parse_hex("#45475a");

        let sidebar = div()
            .flex()
            .flex_col()
            .w(px(250.0))
            .h_full()
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(border)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(text)
                            .child("Zelkova"),
                    )
                    .child(
                        div()
                            .cursor(gpui::CursorStyle::PointingHand)
                            .text_color(text_dim)
                            .text_sm()
                            .child("+")
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    this.handle_create_note(&CreateNote, _window, cx);
                                }),
                            ),
                    ),
            )
            .children(self.notes.iter().enumerate().map(|(i, note)| {
                let is_selected = self.selected == Some(i);
                let note_bg = if is_selected {
                    selection_bg
                } else {
                    sidebar_bg
                };
                let display_title = if note.title.is_empty() {
                    "Untitled"
                } else {
                    &note.title
                };
                let title_color = if note.title.is_empty() {
                    text_dim
                } else {
                    text
                };
                div()
                    .px_3()
                    .py_1()
                    .bg(note_bg)
                    .text_color(title_color)
                    .text_xs()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .child(display_title.to_string())
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |this, _event, _window, cx| {
                            this.selected = Some(i);
                            let path = this.notes[i].path.clone();
                            this.pane_manager.update(cx, |pm, cx| pm.open_tab(path, cx));
                        }),
                    )
            }));

        let mut main = div()
            .flex()
            .flex_row()
            .size_full()
            .bg(bg)
            .key_context("ZelkovaApp")
            .on_action(cx.listener(ZelkovaApp::handle_open_command_palette))
            .on_action(cx.listener(ZelkovaApp::handle_toggle_sidebar))
            .on_action(cx.listener(ZelkovaApp::handle_quit))
            .on_action(cx.listener(ZelkovaApp::handle_cancel))
            .on_action(cx.listener(ZelkovaApp::handle_move_up))
            .on_action(cx.listener(ZelkovaApp::handle_move_down))
            .on_action(cx.listener(ZelkovaApp::handle_confirm))
            .on_action(cx.listener(ZelkovaApp::handle_create_note))
            .on_action(cx.listener(ZelkovaApp::handle_save));

        if self.sidebar_visible {
            main = main.child(sidebar);
        }
        main = main.child(div().flex().flex_col().flex_1().h_full().child(pane));

        if let Some(ref palette) = self.command_palette {
            main = main.child(palette.clone());
        }

        main
    }
}

fn main() {
    let config = AppConfig::load().unwrap_or_default();
    let keymap_config = zelkova_config::KeymapConfig::load().unwrap_or_default();

    Application::new().run(move |cx: &mut App| {
        let bindings = keymap::build_bindings(&keymap_config);
        cx.bind_keys(bindings);

        let bounds = Bounds::centered(None, size(px(1024.0), px(768.0)), cx);
        let config_clone = config.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(SharedString::from("Zelkova")),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|cx| ZelkovaApp::new(config_clone.clone(), cx)),
        )
        .unwrap();
        cx.activate(true);
    });
}
