mod command_palette;
mod editor;
mod keymap;
mod pane;
mod preview;

use std::collections::HashSet;
use std::path::PathBuf;

use gpui::{
    App, Application, Bounds, Context, Entity, SharedString, Subscription, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, size,
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
    folders: Vec<FolderEntry>,
    mappings: Vec<MappingEntry>,
    expanded: HashSet<uuid::Uuid>,
    selected: Option<usize>,
    sidebar_visible: bool,
    command_palette: Option<Entity<command_palette::CommandPalette>>,
    pane_manager: Entity<pane::PaneManager>,
    config: AppConfig,
    ui_colors: zelkova_config::UiColors,
    _pane_subscription: Option<Subscription>,
}

struct NoteEntry {
    id: String,
    title: String,
    path: PathBuf,
}

struct FolderEntry {
    id: uuid::Uuid,
    name: String,
    parent: Option<uuid::Uuid>,
}

struct MappingEntry {
    note_id: uuid::Uuid,
    folder_id: uuid::Uuid,
}

impl ZelkovaApp {
    fn new(config: AppConfig, cx: &mut App) -> Self {
        let mut notes = Vec::new();
        let mut folders = Vec::new();
        let mut mappings = Vec::new();

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
            if let Ok(result) = client.list_tree() {
                folders = result
                    .folders
                    .into_iter()
                    .map(|f| FolderEntry {
                        id: f.id,
                        name: f.name,
                        parent: f.parent,
                    })
                    .collect();
                mappings = result
                    .mappings
                    .into_iter()
                    .map(|m| MappingEntry {
                        note_id: m.note_id,
                        folder_id: m.folder_id,
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

        // Expand all folders by default
        let expanded: HashSet<uuid::Uuid> = folders.iter().map(|f| f.id).collect();

        Self {
            notes,
            folders,
            mappings,
            expanded,
            selected: None,
            sidebar_visible: true,
            command_palette: None,
            pane_manager,
            config,
            ui_colors,
            _pane_subscription: None,
        }
    }

    fn handle_open_command_palette(
        &mut self,
        _: &OpenCommandPalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.is_none() {
            let folder_names: Vec<String> = self.folders.iter().map(|f| f.name.clone()).collect();
            let palette = cx.new(|cx| command_palette::CommandPalette::new(&folder_names, cx));
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
        if let Some(ref palette) = self.command_palette {
            let should_close = palette.update(cx, |p, _| p.handle_back());
            if should_close {
                self.command_palette = None;
            }
        }
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
            cx.notify();
            return;
        }
    }

    fn handle_move_down(&mut self, _: &MoveDown, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.move_selection_down());
            cx.notify();
            return;
        }
    }

    fn handle_insert_newline(
        &mut self,
        _: &InsertNewline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.is_some() {
            self.handle_confirm(&Confirm, window, cx);
            return;
        }
    }

    fn handle_confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            let result = palette.update(cx, |p, _| p.handle_confirm());
            if let Some((label, args)) = result {
                self.execute_command(&label, &args, window, cx);
                self.command_palette = None;
            }
            cx.notify();
            return;
        }
        // Sidebar note selection
        if let Some(sel) = self.selected {
            if let Some(note) = self.notes.get(sel) {
                let path = note.path.clone();
                self.pane_manager.update(cx, |pm, cx| pm.open_tab(path, cx));
                cx.notify();
            }
        }
    }

    fn handle_create_note(&mut self, _: &CreateNote, _window: &mut Window, cx: &mut Context<Self>) {
        if self.config.daemon.socket_path.exists() {
            let client = zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
            if let Ok(result) = client.create_note(None, Vec::new()) {
                let path = result.path.clone();
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

    fn execute_command(
        &mut self,
        label: &str,
        args: &[Option<String>],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match label {
            "Create Note" => {
                let title = args.first().and_then(|a| a.as_deref());
                if self.config.daemon.socket_path.exists() {
                    let client =
                        zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
                    if let Ok(result) = client.create_note(title, Vec::new()) {
                        let path = result.path.clone();
                        self.notes.push(NoteEntry {
                            id: result.id.to_string(),
                            title: result.title.clone(),
                            path: result.path,
                        });
                        self.pane_manager.update(cx, |pm, cx| pm.open_tab(path, cx));
                    }
                }
            }
            "Create Folder" => {
                let name = args
                    .first()
                    .and_then(|a| a.as_deref())
                    .unwrap_or("New Folder");
                let parent_name = args.get(1).and_then(|a| a.as_deref());
                let parent_id = if parent_name == Some("(root)") || parent_name.is_none() {
                    None
                } else {
                    self.folders
                        .iter()
                        .find(|f| Some(f.name.as_str()) == parent_name)
                        .map(|f| f.id)
                };
                if self.config.daemon.socket_path.exists() {
                    let client =
                        zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
                    if let Ok(result) = client.create_folder(name, parent_id) {
                        self.expanded.insert(result.id);
                        self.refresh_folders();
                    }
                }
            }
            "Move to Folder" => {
                let folder_name = args.first().and_then(|a| a.as_deref());
                let folder_id = if folder_name == Some("(root)") || folder_name.is_none() {
                    None
                } else {
                    self.folders
                        .iter()
                        .find(|f| Some(f.name.as_str()) == folder_name)
                        .map(|f| f.id)
                };
                // Move the currently selected note
                if let Some(sel) = self.selected {
                    if let Some(note) = self.notes.get(sel) {
                        if let Ok(note_id) = uuid::Uuid::parse_str(&note.id) {
                            if self.config.daemon.socket_path.exists() {
                                let client = zelkova_rpc::client::RpcClient::new(
                                    &self.config.daemon.socket_path,
                                );
                                if client.move_note(note_id, folder_id).is_ok() {
                                    self.refresh_folders();
                                }
                            }
                        }
                    }
                }
            }
            "Delete Folder" => {
                let folder_name = args.first().and_then(|a| a.as_deref());
                let content_choice = args
                    .get(1)
                    .and_then(|a| a.as_deref())
                    .unwrap_or("Move notes to root");
                let confirmation = args.get(2).and_then(|a| a.as_deref()).unwrap_or("Cancel");
                if confirmation != "Yes, delete" {
                    return;
                }
                let cascade = content_choice == "Delete notes too";
                let folder_id = self
                    .folders
                    .iter()
                    .find(|f| Some(f.name.as_str()) == folder_name)
                    .map(|f| f.id);
                if let Some(folder_id) = folder_id {
                    if self.config.daemon.socket_path.exists() {
                        let client =
                            zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
                        if client.delete_folder(folder_id, cascade).is_ok() {
                            self.expanded.remove(&folder_id);
                            self.refresh_folders();
                            if cascade {
                                self.refresh_notes();
                            }
                        }
                    }
                }
            }
            "Rename Folder" => {
                let folder_name = args.first().and_then(|a| a.as_deref());
                let new_name = args.get(1).and_then(|a| a.as_deref()).unwrap_or("");
                let folder_id = self
                    .folders
                    .iter()
                    .find(|f| Some(f.name.as_str()) == folder_name)
                    .map(|f| f.id);
                if let Some(folder_id) = folder_id {
                    if self.config.daemon.socket_path.exists() {
                        let client =
                            zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
                        if client.rename_folder(folder_id, new_name).is_ok() {
                            self.refresh_folders();
                        }
                    }
                }
            }
            "Toggle Sidebar" => {
                self.sidebar_visible = !self.sidebar_visible;
            }
            "Save Note" => {
                self.handle_save(&SaveNote, window, cx);
            }
            "Quit" => {
                cx.quit();
            }
            _ => {}
        }
    }

    fn refresh_folders(&mut self) {
        if self.config.daemon.socket_path.exists() {
            let client = zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
            if let Ok(result) = client.list_tree() {
                self.folders = result
                    .folders
                    .into_iter()
                    .map(|f| FolderEntry {
                        id: f.id,
                        name: f.name,
                        parent: f.parent,
                    })
                    .collect();
                self.mappings = result
                    .mappings
                    .into_iter()
                    .map(|m| MappingEntry {
                        note_id: m.note_id,
                        folder_id: m.folder_id,
                    })
                    .collect();
            }
        }
    }

    fn refresh_notes(&mut self) {
        if self.config.daemon.socket_path.exists() {
            let client = zelkova_rpc::client::RpcClient::new(&self.config.daemon.socket_path);
            if let Ok(result) = client.list_notes(None) {
                self.notes = result
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
    }

    fn render_sidebar_tree(
        &mut self,
        parent_folder: Option<uuid::Uuid>,
        depth: usize,
        sidebar_bg: gpui::Hsla,
        text: gpui::Hsla,
        text_dim: gpui::Hsla,
        selection_bg: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let mut elements = Vec::new();
        let indent = px(16.0) * depth as f32;

        // Collect folder data at this level
        let folders_at_level: Vec<(uuid::Uuid, String, bool)> = self
            .folders
            .iter()
            .filter(|f| f.parent == parent_folder)
            .map(|f| (f.id, f.name.clone(), self.expanded.contains(&f.id)))
            .collect();

        // Snapshot data needed for note rendering
        let mappings_snapshot: Vec<(uuid::Uuid, usize)> = self
            .mappings
            .iter()
            .filter_map(|m| {
                let idx = self
                    .notes
                    .iter()
                    .position(|n| n.id == m.note_id.to_string())?;
                Some((m.folder_id, idx))
            })
            .collect();

        let notes_snapshot: Vec<(usize, String, bool)> = self
            .notes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                (
                    i,
                    if n.title.is_empty() {
                        "Untitled".to_string()
                    } else {
                        n.title.clone()
                    },
                    n.title.is_empty(),
                )
            })
            .collect();

        let selected = self.selected;

        for (folder_id, folder_name, is_expanded) in &folders_at_level {
            let arrow = if *is_expanded { "▾" } else { "▸" };
            let fid = *folder_id;

            let folder_row = div()
                .px(px(8.0))
                .pl(indent)
                .py_1()
                .text_color(text)
                .text_xs()
                .cursor(gpui::CursorStyle::PointingHand)
                .child(format!("{} {}", arrow, folder_name))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _event, _window, cx| {
                        if this.expanded.contains(&fid) {
                            this.expanded.remove(&fid);
                        } else {
                            this.expanded.insert(fid);
                        }
                        cx.notify();
                    }),
                );
            elements.push(folder_row.into_any_element());

            if *is_expanded {
                // Notes in this folder
                for &(map_folder, note_idx) in &mappings_snapshot {
                    if map_folder != fid {
                        continue;
                    }
                    let (idx, display_title, is_empty) = &notes_snapshot[note_idx];
                    let is_selected = selected == Some(*idx);
                    let note_bg = if is_selected {
                        selection_bg
                    } else {
                        sidebar_bg
                    };
                    let title_color = if *is_empty { text_dim } else { text };
                    let note_row = div()
                        .px(px(8.0))
                        .pl(indent + px(16.0))
                        .py_1()
                        .bg(note_bg)
                        .text_color(title_color)
                        .text_xs()
                        .cursor(gpui::CursorStyle::PointingHand)
                        .child(display_title.clone())
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |this, _event, _window, cx| {
                                this.selected = Some(note_idx);
                                let path = this.notes[note_idx].path.clone();
                                this.pane_manager.update(cx, |pm, cx| pm.open_tab(path, cx));
                                cx.notify();
                            }),
                        );
                    elements.push(note_row.into_any_element());
                }

                // Sub-folders (recursive)
                let mut sub_tree = self.render_sidebar_tree(
                    Some(fid),
                    depth + 1,
                    sidebar_bg,
                    text,
                    text_dim,
                    selection_bg,
                    cx,
                );
                elements.append(&mut sub_tree);
            }
        }

        // At root level, render unmapped notes
        if parent_folder.is_none() {
            let mapped_note_indices: HashSet<usize> =
                mappings_snapshot.iter().map(|&(_, idx)| idx).collect();
            for (idx, display_title, is_empty) in &notes_snapshot {
                if mapped_note_indices.contains(idx) {
                    continue;
                }
                let is_selected = selected == Some(*idx);
                let note_bg = if is_selected {
                    selection_bg
                } else {
                    sidebar_bg
                };
                let title_color = if *is_empty { text_dim } else { text };
                let ni = *idx;
                let dt = display_title.clone();
                let note_row = div()
                    .px(px(8.0))
                    .py_1()
                    .bg(note_bg)
                    .text_color(title_color)
                    .text_xs()
                    .cursor(gpui::CursorStyle::PointingHand)
                    .child(dt)
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |this, _event, _window, cx| {
                            this.selected = Some(ni);
                            let path = this.notes[ni].path.clone();
                            this.pane_manager.update(cx, |pm, cx| pm.open_tab(path, cx));
                            cx.notify();
                        }),
                    );
                elements.push(note_row.into_any_element());
            }
        }

        elements
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
            .children(self.render_sidebar_tree(
                None,
                0,
                sidebar_bg,
                text,
                text_dim,
                selection_bg,
                cx,
            ));

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
            .on_action(cx.listener(ZelkovaApp::handle_insert_newline))
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
            |_, cx| {
                cx.new(|cx| {
                    let mut app = ZelkovaApp::new(config_clone.clone(), cx);
                    // Observe PaneManager to sync sidebar titles in real-time
                    let sub = cx.observe(&app.pane_manager, |this: &mut ZelkovaApp, _pane, cx| {
                        let (path, title) = this.pane_manager.read(cx).active_editor_title(cx);
                        if let (Some(path), Some(title)) = (path, title) {
                            for note in &mut this.notes {
                                if note.path == path {
                                    if note.title != title {
                                        note.title = title;
                                    }
                                    break;
                                }
                            }
                        }
                    });
                    app._pane_subscription = Some(sub);
                    app
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
