use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, SharedString, Subscription,
    Window, div, prelude::*, px,
};
use gpui_component::Selectable;
use gpui_component::resizable::ResizableState;
use gpui_component::tab::{Tab, TabBar};
use zelkova_config::UiColors;

use crate::editor::{Editor, parse_hex};
use crate::pane::{
    PaneId, PaneLeaf, PaneNode, SplitDirection, ViewMode, close_leaf_in_node, create_empty_leaf,
    render_pane_node,
};
use crate::preview::Preview;
use crate::{
    ClosePane, NewTab, NextPane, NextTab, PrevPane, PrevTab, SplitPaneDown, SplitPaneRight,
    ToggleViewMode,
};

pub struct TabWorkspace {
    root: PaneNode,
    focused: PaneId,
}

impl TabWorkspace {
    fn new(leaf: PaneLeaf) -> Self {
        let id = leaf.id;
        Self {
            root: PaneNode::Leaf(leaf),
            focused: id,
        }
    }

    pub fn title(&self, cx: &App) -> String {
        if let Some(leaf) = self.root.find_leaf(self.focused) {
            if leaf.file_path.is_some() {
                return leaf.editor.read(cx).title().to_string();
            }
        }
        "Empty".to_string()
    }

    pub fn pane_count(&self) -> usize {
        self.root.collect_leaves().len()
    }

    pub fn find_file(&self, path: &PathBuf) -> Option<PaneId> {
        self.root.find_file(path)
    }
}

pub struct TabManager {
    tabs: Vec<TabWorkspace>,
    active_tab: usize,
    next_id: usize,
    focus_handle: FocusHandle,
    ui: UiColors,
    socket_path: Option<PathBuf>,
    editor_wrap: bool,
    preview_wrap: bool,
    show_pane_count: bool,
    _editor_subscriptions: Vec<Subscription>,
}

impl TabManager {
    pub fn new(cx: &mut App) -> Self {
        let initial_id = PaneId(0);
        let editor = cx.new(|cx| Editor::new(cx));
        let preview = cx.new(|cx| Preview::from_markdown("", None, cx));

        let leaf = PaneLeaf {
            id: initial_id,
            file_path: None,
            title: String::new(),
            editor,
            preview,
            view_mode: ViewMode::Editor,
        };

        Self {
            tabs: vec![TabWorkspace::new(leaf)],
            active_tab: 0,
            next_id: 1,
            focus_handle: cx.focus_handle(),
            ui: UiColors::default(),
            socket_path: None,
            editor_wrap: true,
            preview_wrap: true,
            show_pane_count: true,
            _editor_subscriptions: Vec::new(),
        }
    }

    pub fn set_socket_path(&mut self, path: PathBuf) {
        self.socket_path = Some(path);
    }

    pub fn set_theme(&mut self, ui: UiColors) {
        self.ui = ui;
    }

    pub fn set_wrap(&mut self, editor_wrap: bool, preview_wrap: bool, cx: &mut App) {
        self.editor_wrap = editor_wrap;
        self.preview_wrap = preview_wrap;
        for tab in &mut self.tabs {
            Self::for_each_leaf_mut(
                &mut tab.root,
                |leaf, cx| {
                    leaf.editor.update(cx, |ed, _| ed.set_wrap(editor_wrap));
                    leaf.preview.update(cx, |p, _| p.set_wrap(preview_wrap));
                },
                cx,
            );
        }
    }

    fn for_each_leaf_mut(node: &mut PaneNode, f: impl Fn(&mut PaneLeaf, &mut App), cx: &mut App) {
        fn apply(node: &mut PaneNode, f: &dyn Fn(&mut PaneLeaf, &mut App), cx: &mut App) {
            match node {
                PaneNode::Leaf(leaf) => f(leaf, cx),
                PaneNode::Split { children, .. } => {
                    apply(&mut children.0, f, cx);
                    apply(&mut children.1, f, cx);
                }
            }
        }
        apply(node, &f, cx);
    }

    fn active_tab(&self) -> &TabWorkspace {
        &self.tabs[self.active_tab]
    }

    fn active_tab_mut(&mut self) -> &mut TabWorkspace {
        &mut self.tabs[self.active_tab]
    }

    fn auto_save_focused(&self, cx: &mut Context<Self>) {
        let tab = self.active_tab();
        if let Some(leaf) = tab.root.find_leaf(tab.focused) {
            if leaf.file_path.is_some() && leaf.editor.read(cx).is_dirty() {
                let editor = leaf.editor.clone();
                editor.update(cx, |ed, _| ed.save_to_disk());
            }
        }
    }

    /// Search all tabs for a file. Returns (tab_index, pane_id) if found.
    #[allow(dead_code)]
    pub fn find_file_in_all_tabs(&self, path: &PathBuf) -> Option<(usize, PaneId)> {
        for (i, tab) in self.tabs.iter().enumerate() {
            if let Some(pane_id) = tab.find_file(path) {
                return Some((i, pane_id));
            }
        }
        None
    }

    pub fn open_in_focused(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // Cross-tab search: if file is open in another tab, switch to it
        for (i, tab) in self.tabs.iter().enumerate() {
            if let Some(pane_id) = tab.find_file(&path) {
                self.active_tab = i;
                self.tabs[i].focused = pane_id;
                cx.notify();
                return;
            }
        }

        // Auto-save focused leaf before replacing
        self.auto_save_focused(cx);

        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("file_stem is valid because PathBuf came from a valid file path")
            .to_string();

        let editor = cx.new(|cx| match Editor::load(path.clone(), cx) {
            Ok(e) => e,
            Err(_) => Editor::new(cx),
        });
        if let Some(ref socket) = self.socket_path {
            editor.update(cx, |ed, _| ed.set_socket_path(socket.clone()));
        }
        editor.update(cx, |ed, _| ed.set_wrap(self.editor_wrap));

        let text = editor.read(cx).text().to_string();
        let preview = cx.new(|cx| Preview::from_markdown(&text, Some(path.clone()), cx));
        preview.update(cx, |p, _| p.set_wrap(self.preview_wrap));

        let editor_clone = editor.clone();
        let sub = cx.observe(&editor, move |this, editor, cx| {
            this.sync_editor_to_leaf(&editor, cx);
            cx.notify();
        });
        self._editor_subscriptions.push(sub);

        let tab = self.active_tab_mut();
        if let Some(leaf) = tab.root.find_leaf_mut(tab.focused) {
            leaf.file_path = Some(path);
            leaf.title = title;
            leaf.editor = editor_clone;
            leaf.preview = preview;
            leaf.view_mode = ViewMode::Editor;
        }

        cx.notify();
    }

    fn sync_editor_to_leaf(&mut self, editor: &Entity<Editor>, cx: &mut Context<Self>) {
        for tab in &mut self.tabs {
            if let Some(leaf) = tab.root.find_leaf_by_editor_mut(editor) {
                let new_title = editor.read(cx).title().to_string();
                if leaf.title != new_title {
                    leaf.title = new_title;
                }
                let text = editor.read(cx).text().to_string();
                leaf.preview.update(cx, |p, _| p.update_content(&text));
                return;
            }
        }
    }

    // -- Pane operations (delegate to active tab) --

    pub fn handle_split_right(
        &mut self,
        _: &SplitPaneRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_focused(SplitDirection::Horizontal, cx);
    }

    pub fn handle_split_down(
        &mut self,
        _: &SplitPaneDown,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_focused(SplitDirection::Vertical, cx);
    }

    fn split_focused(&mut self, direction: SplitDirection, cx: &mut Context<Self>) {
        let focused = self.tabs[self.active_tab].focused;
        let new_id = PaneId(self.next_id);
        self.next_id += 1;
        let split_id = PaneId(self.next_id);
        self.next_id += 1;

        let socket_path = self.socket_path.clone();
        let editor_wrap = self.editor_wrap;
        let preview_wrap = self.preview_wrap;

        let new_leaf = create_empty_leaf(new_id, socket_path, editor_wrap, preview_wrap, cx);
        let resize_state = cx.new(|_| ResizableState::default());

        let tab = &mut self.tabs[self.active_tab];
        if let PaneNode::Leaf(ref old_leaf) = tab.root {
            if old_leaf.id == focused {
                let old_leaf_clone = PaneLeaf {
                    id: old_leaf.id,
                    file_path: old_leaf.file_path.clone(),
                    title: old_leaf.title.clone(),
                    editor: old_leaf.editor.clone(),
                    preview: old_leaf.preview.clone(),
                    view_mode: old_leaf.view_mode,
                };
                tab.root = PaneNode::Split {
                    id: split_id,
                    direction,
                    children: Box::new((PaneNode::Leaf(old_leaf_clone), PaneNode::Leaf(new_leaf))),
                    resize_state,
                };
                tab.focused = new_id;
                cx.notify();
                return;
            }
        }

        let tab = &mut self.tabs[self.active_tab];
        Self::split_in_tree(
            &mut tab.root,
            focused,
            direction,
            new_leaf,
            split_id,
            resize_state,
        );
        tab.focused = new_id;
        cx.notify();
    }

    fn split_in_tree(
        node: &mut PaneNode,
        target_id: PaneId,
        direction: SplitDirection,
        new_leaf: PaneLeaf,
        split_id: PaneId,
        resize_state: Entity<ResizableState>,
    ) {
        match node {
            PaneNode::Leaf(leaf) if leaf.id == target_id => {
                let old_leaf = PaneLeaf {
                    id: leaf.id,
                    file_path: leaf.file_path.clone(),
                    title: leaf.title.clone(),
                    editor: leaf.editor.clone(),
                    preview: leaf.preview.clone(),
                    view_mode: leaf.view_mode,
                };
                *node = PaneNode::Split {
                    id: split_id,
                    direction,
                    children: Box::new((PaneNode::Leaf(old_leaf), PaneNode::Leaf(new_leaf))),
                    resize_state,
                };
            }
            PaneNode::Split { children, .. } => {
                Self::split_in_tree(
                    &mut children.0,
                    target_id,
                    direction,
                    new_leaf.clone(),
                    split_id,
                    resize_state.clone(),
                );
                Self::split_in_tree(
                    &mut children.1,
                    target_id,
                    direction,
                    new_leaf,
                    split_id,
                    resize_state,
                );
            }
            _ => {}
        }
    }

    pub fn handle_close_pane(
        &mut self,
        _: &ClosePane,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.auto_save_focused(cx);

        let is_single_leaf = self.tabs[self.active_tab].root.is_single_leaf();

        // Single leaf in the tab
        if is_single_leaf {
            // Last tab with single pane: clear file
            if self.tabs.len() == 1 {
                let tab = &mut self.tabs[0];
                if let PaneNode::Leaf(leaf) = &mut tab.root {
                    if leaf.editor.read(cx).is_dirty() {
                        leaf.editor.update(cx, |ed, _| ed.save_to_disk());
                    }
                    leaf.file_path = None;
                    leaf.title = String::new();
                    leaf.editor = cx.new(|cx| Editor::new(cx));
                    leaf.preview = cx.new(|cx| Preview::from_markdown("", None, cx));
                    leaf.view_mode = ViewMode::Editor;
                }
                cx.notify();
                return;
            }

            // Multiple tabs: remove this tab
            let removed_idx = self.active_tab;
            self.tabs.remove(removed_idx);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            cx.notify();
            return;
        }

        // Multiple panes in tab: close focused pane
        let focused = self.tabs[self.active_tab].focused;
        let tab = &mut self.tabs[self.active_tab];
        close_leaf_in_node(&mut tab.root, focused);
        tab.focused = tab.root.first_leaf_id();
        cx.notify();
    }

    pub fn handle_next_pane(&mut self, _: &NextPane, _window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.active_tab();
        let leaves = tab.root.collect_leaves();
        if leaves.len() <= 1 {
            return;
        }
        self.auto_save_focused(cx);
        if let Some(pos) = leaves.iter().position(|&id| id == tab.focused) {
            self.active_tab_mut().focused = leaves[(pos + 1) % leaves.len()];
        }
        cx.notify();
    }

    pub fn handle_prev_pane(&mut self, _: &PrevPane, _window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.active_tab();
        let leaves = tab.root.collect_leaves();
        if leaves.len() <= 1 {
            return;
        }
        self.auto_save_focused(cx);
        if let Some(pos) = leaves.iter().position(|&id| id == tab.focused) {
            self.active_tab_mut().focused = if pos == 0 {
                leaves[leaves.len() - 1]
            } else {
                leaves[pos - 1]
            };
        }
        cx.notify();
    }

    pub fn handle_toggle_view(
        &mut self,
        _: &ToggleViewMode,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = self.active_tab_mut();
        if let Some(leaf) = tab.root.find_leaf_mut(tab.focused) {
            leaf.view_mode = match leaf.view_mode {
                ViewMode::Editor => ViewMode::Split,
                ViewMode::Split => ViewMode::Preview,
                ViewMode::Preview => ViewMode::Editor,
            };
            let text = leaf.editor.read(cx).text().to_string();
            leaf.preview.update(cx, |p, _| p.update_content(&text));
        }
        cx.notify();
    }

    // -- Tab operations --

    pub fn handle_new_tab(&mut self, _: &NewTab, _window: &mut Window, cx: &mut Context<Self>) {
        self.auto_save_focused(cx);

        let new_id = PaneId(self.next_id);
        self.next_id += 1;

        let leaf = create_empty_leaf(
            new_id,
            self.socket_path.clone(),
            self.editor_wrap,
            self.preview_wrap,
            cx,
        );

        self.tabs.push(TabWorkspace::new(leaf));
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }

    pub fn handle_next_tab(&mut self, _: &NextTab, _window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.auto_save_focused(cx);
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
        cx.notify();
    }

    pub fn handle_prev_tab(&mut self, _: &PrevTab, _window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.auto_save_focused(cx);
        self.active_tab = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
        cx.notify();
    }

    pub fn active_editor_title(&self, cx: &App) -> (Option<PathBuf>, Option<String>) {
        let tab = self.active_tab();
        if let Some(leaf) = tab.root.find_leaf(tab.focused) {
            let path = leaf.file_path.clone();
            let title = leaf.editor.read(cx).title().to_string();
            return (path, Some(title));
        }
        (None, None)
    }
}

impl Focusable for TabManager {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TabManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = parse_hex(&self.ui.border);
        let text_dim = parse_hex(&self.ui.text_dim);
        let active_tab = self.active_tab;
        let show_pane_count = self.show_pane_count;

        // Capture entity early for closures
        let tm_for_bar = cx.entity().clone();
        let tm_for_focus = cx.entity().clone();

        // Build tab bar
        let tab_bar_items: Vec<Tab> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let label = tab.title(cx);
                let display = if show_pane_count && tab.pane_count() > 1 {
                    format!("{} ({})", label, tab.pane_count())
                } else {
                    label
                };
                Tab::new()
                    .label(SharedString::from(display))
                    .selected(i == active_tab)
            })
            .collect();

        let tab_bar = TabBar::new("workspace-tabs")
            .selected_index(active_tab)
            .on_click(move |index, _window, cx| {
                tm_for_bar.update(cx, |tm, cx| {
                    tm.active_tab = *index;
                    cx.notify();
                });
            })
            .children(tab_bar_items);

        // Build pane tree for active tab
        let tab = self.active_tab_mut();
        let focused = tab.focused;

        let on_focus: Rc<dyn Fn(PaneId, &mut App)> = Rc::new(move |leaf_id, cx| {
            tm_for_focus.update(cx, |tm, cx| {
                tm.active_tab_mut().focused = leaf_id;
                cx.notify();
            });
        });

        // Focus the active view
        let tab = self.active_tab();
        if let Some(leaf) = tab.root.find_leaf(tab.focused) {
            match leaf.view_mode {
                ViewMode::Editor => leaf.editor.focus_handle(cx).focus(window),
                ViewMode::Preview => leaf.preview.focus_handle(cx).focus(window),
                ViewMode::Split => leaf.editor.focus_handle(cx).focus(window),
            }
        } else {
            self.focus_handle.focus(window);
        }

        let mut main = div()
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle)
            .child(tab_bar);

        // Single leaf: render header + content as direct children (no wrapper)
        // Split: use recursive rendering inside a container
        match &self.tabs[self.active_tab].root {
            PaneNode::Leaf(leaf) => {
                let header = div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .py(px(2.0))
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .text_xs()
                            .text_color(text_dim)
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(if leaf.file_path.is_some() {
                                leaf.title.clone()
                            } else {
                                String::new()
                            }),
                    );

                let content = if leaf.file_path.is_none() {
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(text_dim)
                        .text_sm()
                        .child("Open or Create Note")
                        .into_any_element()
                } else {
                    match leaf.view_mode {
                        ViewMode::Editor => div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(leaf.editor.clone())
                            .into_any_element(),
                        ViewMode::Preview => div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .child(leaf.preview.clone())
                            .into_any_element(),
                        ViewMode::Split => div()
                            .flex()
                            .flex_row()
                            .flex_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .child(leaf.editor.clone()),
                            )
                            .child(div().w(px(1.0)).bg(border))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .overflow_hidden()
                                    .child(leaf.preview.clone()),
                            )
                            .into_any_element(),
                    }
                };

                main = main.child(header).child(content);
            }
            PaneNode::Split { .. } => {
                let tree = render_pane_node(
                    &self.tabs[self.active_tab].root,
                    focused,
                    border,
                    text_dim,
                    on_focus,
                );
                main = main.child(div().flex().flex_col().flex_1().min_h(px(0.0)).child(tree));
            }
        }

        main.on_action(cx.listener(TabManager::handle_split_right))
            .on_action(cx.listener(TabManager::handle_split_down))
            .on_action(cx.listener(TabManager::handle_close_pane))
            .on_action(cx.listener(TabManager::handle_next_pane))
            .on_action(cx.listener(TabManager::handle_prev_pane))
            .on_action(cx.listener(TabManager::handle_toggle_view))
            .on_action(cx.listener(TabManager::handle_new_tab))
            .on_action(cx.listener(TabManager::handle_next_tab))
            .on_action(cx.listener(TabManager::handle_prev_tab))
    }
}
