mod command_palette;
mod editor;
mod keymap;
mod pane;
mod preview;
mod tab;

use std::collections::HashSet;
use std::path::PathBuf;

use gpui::{
    App, Application, Bounds, Context, Entity, SharedString, Subscription, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_component::Root;
use gpui_component::resizable::{ResizableState, h_resizable, resizable_panel};
use gpui_component::sidebar::{
    Sidebar, SidebarHeader, SidebarMenu, SidebarMenuItem, SidebarToggleButton,
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
        ResizeSidebarLeft,
        ResizeSidebarRight,
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
        SplitPaneRight,
        SplitPaneDown,
        ClosePane,
        NewTab,
        NextTab,
        PrevTab,
        Undo,
        Redo,
        Confirm,
        Cancel,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Copy,
        Paste,
        Cut,
    ]
);

struct ZelkovaApp {
    notes: Vec<NoteEntry>,
    folders: Vec<FolderEntry>,
    mappings: Vec<MappingEntry>,
    expanded: HashSet<uuid::Uuid>,
    sidebar_visible: bool,
    command_palette: Option<Entity<command_palette::CommandPalette>>,
    tab_manager: Entity<tab::TabManager>,
    sidebar_resize_state: Entity<ResizableState>,
    sidebar_width: gpui::Pixels,
    config: AppConfig,
    _tab_subscription: Option<Subscription>,
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
    fn rpc_client(&self) -> Option<zelkova_rpc::client::RpcClient> {
        if self.config.daemon.socket_path.exists() {
            Some(zelkova_rpc::client::RpcClient::new(
                &self.config.daemon.socket_path,
            ))
        } else {
            None
        }
    }

    fn find_note_by_title(&self, title: Option<&str>) -> Option<&NoteEntry> {
        self.notes.iter().find(|n| {
            title == Some(n.title.as_str()) || (n.title.is_empty() && title == Some("Untitled"))
        })
    }

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

        let tab_manager = cx.new(|cx| {
            let mut tm = tab::TabManager::new(cx);
            tm.set_socket_path(config.daemon.socket_path.clone());
            tm.set_theme(ui_colors.clone());
            tm.set_wrap(config.editor.wrap, config.preview.wrap, cx);
            tm
        });

        // Expand all folders by default
        let expanded: HashSet<uuid::Uuid> = folders.iter().map(|f| f.id).collect();

        let sidebar_resize_state = cx.new(|_| ResizableState::default());

        Self {
            notes,
            folders,
            mappings,
            expanded,
            sidebar_visible: true,
            command_palette: None,
            tab_manager,
            sidebar_resize_state,
            sidebar_width: px(220.0),
            config,
            _tab_subscription: None,
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
            let note_titles: Vec<String> = self
                .notes
                .iter()
                .map(|n| {
                    if n.title.is_empty() {
                        "Untitled".to_string()
                    } else {
                        n.title.clone()
                    }
                })
                .collect();
            let palette =
                cx.new(|cx| command_palette::CommandPalette::new(&folder_names, &note_titles, cx));
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

    fn handle_resize_sidebar_left(
        &mut self,
        _: &ResizeSidebarLeft,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidebar_width = (self.sidebar_width - px(20.0)).max(px(150.0));
        self.sidebar_resize_state = cx.new(|_| ResizableState::default());
        cx.notify();
    }

    fn handle_resize_sidebar_right(
        &mut self,
        _: &ResizeSidebarRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sidebar_width = (self.sidebar_width + px(20.0)).min(px(400.0));
        self.sidebar_resize_state = cx.new(|_| ResizableState::default());
        cx.notify();
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
        let (path, title) = self.tab_manager.read(cx).active_editor_title(cx);
        if let (Some(path), Some(title)) = (path, title) {
            for note in &mut self.notes {
                if note.path == path {
                    note.title = title;
                    break;
                }
            }
        }
    }

    fn handle_move_left(&mut self, _: &MoveLeft, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.move_cursor_left());
            cx.notify();
        }
    }

    fn handle_move_right(&mut self, _: &MoveRight, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.move_cursor_right());
            cx.notify();
        }
    }

    fn handle_backspace(&mut self, _: &Backspace, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.handle_backspace());
            cx.notify();
        }
    }

    fn handle_move_up(&mut self, _: &MoveUp, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.move_selection_up());
            cx.notify();
        }
    }

    fn handle_move_down(&mut self, _: &MoveDown, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            palette.update(cx, |p, _| p.move_selection_down());
            cx.notify();
        }
    }

    fn handle_copy(&mut self, _: &Copy, _window: &mut Window, _cx: &mut Context<Self>) {
        // Editor handles via its own on_action; command palette doesn't need copy
    }

    fn handle_paste(&mut self, _: &Paste, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref palette) = self.command_palette {
            if let Some(item) = cx.read_from_clipboard()
                && let Some(text) = item.text()
            {
                palette.update(cx, |p, _| p.paste_text(text.to_string()));
            }
            cx.notify();
        }
    }

    fn handle_cut(&mut self, _: &Cut, _window: &mut Window, _cx: &mut Context<Self>) {
        // Editor handles via its own on_action; command palette doesn't need cut
    }

    fn handle_insert_newline(
        &mut self,
        _: &InsertNewline,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.command_palette.is_some() {
            self.handle_confirm(&Confirm, window, cx);
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
        }
    }

    fn handle_create_note(&mut self, _: &CreateNote, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(client) = self.rpc_client() {
            if let Ok(result) = client.create_note(None, Vec::new()) {
                let path = result.path.clone();
                self.notes.push(NoteEntry {
                    id: result.id.to_string(),
                    title: result.title.clone(),
                    path: result.path,
                });
                self.tab_manager
                    .update(cx, |tm, cx| tm.open_in_focused(path, cx));
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
                if let Some(client) = self.rpc_client() {
                    if let Ok(result) = client.create_note(title, Vec::new()) {
                        let path = result.path.clone();
                        self.notes.push(NoteEntry {
                            id: result.id.to_string(),
                            title: result.title.clone(),
                            path: result.path,
                        });
                        self.tab_manager
                            .update(cx, |tm, cx| tm.open_in_focused(path, cx));
                    }
                }
            }
            "Create Folder" => {
                let name = args
                    .first()
                    .and_then(|a| a.as_deref())
                    .unwrap_or("New Folder");
                let parent_name = args.get(1).and_then(|a| a.as_deref());
                let parent_id = resolve_folder_id(&self.folders, parent_name);
                if let Some(client) = self.rpc_client() {
                    if let Ok(result) = client.create_folder(name, parent_id) {
                        self.expanded.insert(result.id);
                        self.refresh_folders();
                    }
                }
            }
            "Move Note to Folder" => {
                let note_title = args.first().and_then(|a| a.as_deref());
                let dest_name = args.get(1).and_then(|a| a.as_deref());
                let dest_id = resolve_folder_id(&self.folders, dest_name);
                if let Some(note) = self.find_note_by_title(note_title)
                    && let Ok(note_id) = uuid::Uuid::parse_str(&note.id)
                    && let Some(client) = self.rpc_client()
                {
                    if client.move_note(note_id, dest_id).is_ok() {
                        self.refresh_folders();
                    }
                }
            }
            "Move Folder to Folder" => {
                let folder_name = args.first().and_then(|a| a.as_deref());
                let dest_name = args.get(1).and_then(|a| a.as_deref());
                let dest_id = resolve_folder_id(&self.folders, dest_name);
                let folder_id = self
                    .folders
                    .iter()
                    .find(|f| Some(f.name.as_str()) == folder_name)
                    .map(|f| f.id);
                if let Some(folder_id) = folder_id
                    && let Some(client) = self.rpc_client()
                {
                    if client.move_folder(folder_id, dest_id).is_ok() {
                        self.refresh_folders();
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
                if let Some(folder_id) = folder_id
                    && let Some(client) = self.rpc_client()
                {
                    if client.delete_folder(folder_id, cascade).is_ok() {
                        self.expanded.remove(&folder_id);
                        self.refresh_folders();
                        if cascade {
                            self.refresh_notes();
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
                if let Some(folder_id) = folder_id
                    && let Some(client) = self.rpc_client()
                {
                    if client.rename_folder(folder_id, new_name).is_ok() {
                        self.refresh_folders();
                    }
                }
            }
            "Rename Note" => {
                let note_title = args.first().and_then(|a| a.as_deref());
                let new_title = args.get(1).and_then(|a| a.as_deref()).unwrap_or("");
                if let Some(note) = self.find_note_by_title(note_title)
                    && let Ok(note_id) = uuid::Uuid::parse_str(&note.id)
                    && let Some(client) = self.rpc_client()
                {
                    if client.rename_note(note_id, new_title).is_ok() {
                        self.refresh_notes();
                    }
                }
            }
            "Delete Note" => {
                let confirmation = args.get(1).and_then(|a| a.as_deref()).unwrap_or("Cancel");
                if confirmation != "Yes, delete" {
                    return;
                }
                let note_title = args.first().and_then(|a| a.as_deref());
                if let Some(note) = self.find_note_by_title(note_title)
                    && let Ok(note_id) = uuid::Uuid::parse_str(&note.id)
                    && let Some(client) = self.rpc_client()
                {
                    if client.delete_note(note_id).is_ok() {
                        self.refresh_notes();
                        self.refresh_folders();
                    }
                }
            }
            "Toggle Sidebar" => {
                self.sidebar_visible = !self.sidebar_visible;
            }
            "Toggle View Mode" => {
                self.tab_manager.update(cx, |tm, cx| {
                    tm.handle_toggle_view(&ToggleViewMode, window, cx);
                });
            }
            "Split Pane Right" => {
                self.tab_manager.update(cx, |tm, cx| {
                    tm.handle_split_right(&SplitPaneRight, window, cx);
                });
            }
            "Split Pane Down" => {
                self.tab_manager.update(cx, |tm, cx| {
                    tm.handle_split_down(&SplitPaneDown, window, cx);
                });
            }
            "Close Pane" => {
                self.tab_manager.update(cx, |tm, cx| {
                    tm.handle_close_pane(&ClosePane, window, cx);
                });
            }
            "New Tab" => {
                self.tab_manager.update(cx, |tm, cx| {
                    tm.handle_new_tab(cx);
                });
            }
            "Next Tab" => {
                self.tab_manager.update(cx, |tm, cx| {
                    tm.handle_next_tab(cx);
                });
            }
            "Prev Tab" => {
                self.tab_manager.update(cx, |tm, cx| {
                    tm.handle_prev_tab(cx);
                });
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
        if let Some(client) = self.rpc_client() {
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
        if let Some(client) = self.rpc_client() {
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
}

fn build_sidebar_items(
    folders: &[FolderEntry],
    notes: &[NoteEntry],
    mappings: &[MappingEntry],
    expanded: &HashSet<uuid::Uuid>,
    tab_manager: &Entity<tab::TabManager>,
) -> Vec<SidebarMenuItem> {
    let mut items = Vec::new();

    for folder in folders.iter().filter(|f| f.parent.is_none()) {
        items.push(build_folder_item(
            folder,
            folders,
            notes,
            mappings,
            expanded,
            tab_manager,
        ));
    }

    let mapped_note_ids: HashSet<String> = mappings.iter().map(|m| m.note_id.to_string()).collect();
    for note in notes.iter().filter(|n| !mapped_note_ids.contains(&n.id)) {
        items.push(build_note_item(note, tab_manager));
    }

    items
}

fn build_folder_item(
    folder: &FolderEntry,
    all_folders: &[FolderEntry],
    notes: &[NoteEntry],
    mappings: &[MappingEntry],
    expanded: &HashSet<uuid::Uuid>,
    tab_manager: &Entity<tab::TabManager>,
) -> SidebarMenuItem {
    let is_expanded = expanded.contains(&folder.id);
    let mut children: Vec<SidebarMenuItem> = Vec::new();

    for mapping in mappings.iter().filter(|m| m.folder_id == folder.id) {
        if let Some(note) = notes.iter().find(|n| n.id == mapping.note_id.to_string()) {
            children.push(build_note_item(note, tab_manager));
        }
    }

    for sub_folder in all_folders.iter().filter(|f| f.parent == Some(folder.id)) {
        children.push(build_folder_item(
            sub_folder,
            all_folders,
            notes,
            mappings,
            expanded,
            tab_manager,
        ));
    }

    let mut item = SidebarMenuItem::new(folder.name.clone());
    if !children.is_empty() {
        item = item
            .default_open(is_expanded)
            .click_to_open(true)
            .children(children);
    }
    item
}

fn build_note_item(note: &NoteEntry, tab_manager: &Entity<tab::TabManager>) -> SidebarMenuItem {
    let title = if note.title.is_empty() {
        "Untitled".to_string()
    } else {
        note.title.clone()
    };
    let path = note.path.clone();
    let tm = tab_manager.clone();

    SidebarMenuItem::new(title).on_click(move |_event, _window, cx| {
        tm.update(cx, |tm, cx| tm.open_in_focused(path.clone(), cx));
    })
}

fn resolve_folder_id(folders: &[FolderEntry], name: Option<&str>) -> Option<uuid::Uuid> {
    if name == Some("(root)") || name.is_none() {
        None
    } else {
        folders
            .iter()
            .find(|f| Some(f.name.as_str()) == name)
            .map(|f| f.id)
    }
}

impl Render for ZelkovaApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab_mgr = self.tab_manager.clone();

        let sidebar_items = build_sidebar_items(
            &self.folders,
            &self.notes,
            &self.mappings,
            &self.expanded,
            &self.tab_manager,
        );

        let header = SidebarHeader::new().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .w_full()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(SidebarToggleButton::left().on_click(cx.listener(
                            |this, _event, _window, cx| {
                                this.sidebar_visible = false;
                                cx.notify();
                            },
                        )))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::BOLD)
                                .child("Zelkova"),
                        ),
                )
                .child(
                    div()
                        .cursor(gpui::CursorStyle::PointingHand)
                        .text_sm()
                        .child("+")
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _event, _window, cx| {
                                this.handle_create_note(&CreateNote, _window, cx);
                            }),
                        ),
                ),
        );

        let sidebar = Sidebar::left()
            .header(header)
            .child(SidebarMenu::new().children(sidebar_items))
            .w_full();

        let mut main = div()
            .flex()
            .flex_row()
            .size_full()
            .key_context("ZelkovaApp")
            .on_action(cx.listener(ZelkovaApp::handle_open_command_palette))
            .on_action(cx.listener(ZelkovaApp::handle_toggle_sidebar))
            .on_action(cx.listener(ZelkovaApp::handle_resize_sidebar_left))
            .on_action(cx.listener(ZelkovaApp::handle_resize_sidebar_right))
            .on_action(cx.listener(ZelkovaApp::handle_quit))
            .on_action(cx.listener(ZelkovaApp::handle_cancel))
            .on_action(cx.listener(ZelkovaApp::handle_move_left))
            .on_action(cx.listener(ZelkovaApp::handle_move_right))
            .on_action(cx.listener(ZelkovaApp::handle_backspace))
            .on_action(cx.listener(ZelkovaApp::handle_move_up))
            .on_action(cx.listener(ZelkovaApp::handle_move_down))
            .on_action(cx.listener(ZelkovaApp::handle_confirm))
            .on_action(cx.listener(ZelkovaApp::handle_insert_newline))
            .on_action(cx.listener(ZelkovaApp::handle_create_note))
            .on_action(cx.listener(ZelkovaApp::handle_save))
            .on_action(cx.listener(ZelkovaApp::handle_copy))
            .on_action(cx.listener(ZelkovaApp::handle_paste))
            .on_action(cx.listener(ZelkovaApp::handle_cut));

        if self.sidebar_visible {
            let resizable = h_resizable("app-layout")
                .with_state(&self.sidebar_resize_state)
                .child(
                    resizable_panel()
                        .size(self.sidebar_width)
                        .size_range(px(150.0)..px(400.0))
                        .child(sidebar),
                )
                .child(resizable_panel().child(tab_mgr));
            main = main.child(resizable);
        } else {
            let mut content = div()
                .relative()
                .flex()
                .flex_col()
                .flex_1()
                .h_full()
                .child(tab_mgr);

            content = content.child(
                div().absolute().top_0().left_0().p_1().child(
                    SidebarToggleButton::left()
                        .collapsed(true)
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.sidebar_visible = true;
                            cx.notify();
                        })),
                ),
            );

            main = main.child(content);
        }

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
        gpui_component::init(cx);

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
            |window, cx| {
                let app = cx.new(|cx| {
                    let mut app = ZelkovaApp::new(config_clone.clone(), cx);
                    let sub = cx.observe(&app.tab_manager, |this: &mut ZelkovaApp, _tm, cx| {
                        let (path, title) = this.tab_manager.read(cx).active_editor_title(cx);
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
                    app._tab_subscription = Some(sub);
                    app
                });
                cx.new(|cx| Root::new(app, window, cx))
            },
        )
        .expect("window creation is infallible on supported platforms");
        cx.activate(true);
    });
}
