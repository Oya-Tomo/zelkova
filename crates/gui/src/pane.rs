use std::path::PathBuf;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Subscription, Window, div,
    prelude::*, px,
};
use gpui_component::resizable::{ResizableState, h_resizable, resizable_panel, v_resizable};
use zelkova_config::UiColors;

use crate::editor::Editor;
use crate::editor::parse_hex;
use crate::preview::Preview;
use crate::{ClosePane, NextPane, PrevPane, SplitPaneDown, SplitPaneRight, ToggleViewMode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Editor,
    Split,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
pub struct PaneLeaf {
    pub id: PaneId,
    pub file_path: Option<PathBuf>,
    pub title: String,
    pub editor: Entity<Editor>,
    pub preview: Entity<Preview>,
    pub view_mode: ViewMode,
}

#[derive(Clone)]
pub enum PaneNode {
    Leaf(PaneLeaf),
    Split {
        id: PaneId,
        direction: SplitDirection,
        children: Box<(PaneNode, PaneNode)>,
        resize_state: Entity<ResizableState>,
    },
}

impl PaneNode {
    fn find_leaf(&self, id: PaneId) -> Option<&PaneLeaf> {
        match self {
            PaneNode::Leaf(leaf) if leaf.id == id => Some(leaf),
            PaneNode::Leaf(_) => None,
            PaneNode::Split { children, .. } => children
                .0
                .find_leaf(id)
                .or_else(|| children.1.find_leaf(id)),
        }
    }

    fn find_leaf_mut(&mut self, id: PaneId) -> Option<&mut PaneLeaf> {
        match self {
            PaneNode::Leaf(leaf) if leaf.id == id => Some(leaf),
            PaneNode::Leaf(_) => None,
            PaneNode::Split { children, .. } => children
                .0
                .find_leaf_mut(id)
                .or_else(|| children.1.find_leaf_mut(id)),
        }
    }

    fn collect_leaves(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf(leaf) => vec![leaf.id],
            PaneNode::Split { children, .. } => {
                let mut leaves = children.0.collect_leaves();
                leaves.extend(children.1.collect_leaves());
                leaves
            }
        }
    }

    fn find_file(&self, path: &PathBuf) -> Option<PaneId> {
        match self {
            PaneNode::Leaf(leaf) => {
                if leaf.file_path.as_ref() == Some(path) {
                    Some(leaf.id)
                } else {
                    None
                }
            }
            PaneNode::Split { children, .. } => children
                .0
                .find_file(path)
                .or_else(|| children.1.find_file(path)),
        }
    }

    fn find_leaf_by_editor_mut(&mut self, editor: &Entity<Editor>) -> Option<&mut PaneLeaf> {
        match self {
            PaneNode::Leaf(leaf) => {
                if &leaf.editor == editor {
                    Some(leaf)
                } else {
                    None
                }
            }
            PaneNode::Split { children, .. } => children
                .0
                .find_leaf_by_editor_mut(editor)
                .or_else(|| children.1.find_leaf_by_editor_mut(editor)),
        }
    }

    fn first_leaf_id(&self) -> PaneId {
        match self {
            PaneNode::Leaf(leaf) => leaf.id,
            PaneNode::Split { children, .. } => children.0.first_leaf_id(),
        }
    }

    fn is_single_leaf(&self) -> bool {
        matches!(self, PaneNode::Leaf(_))
    }
}

fn close_leaf_in_node(node: &mut PaneNode, target_id: PaneId) -> bool {
    match node {
        PaneNode::Split { children, .. } => {
            let left_is_target =
                matches!(&children.0, PaneNode::Leaf(leaf) if leaf.id == target_id);
            let right_is_target =
                matches!(&children.1, PaneNode::Leaf(leaf) if leaf.id == target_id);

            if left_is_target {
                let replacement = children.1.clone();
                *node = replacement;
                return true;
            }
            if right_is_target {
                let replacement = children.0.clone();
                *node = replacement;
                return true;
            }

            close_leaf_in_node(&mut children.0, target_id)
                || close_leaf_in_node(&mut children.1, target_id)
        }
        _ => false,
    }
}

fn split_leaf_in_node(
    node: &mut PaneNode,
    target_id: PaneId,
    direction: SplitDirection,
    new_id: PaneId,
    split_id: PaneId,
    cx: &mut Context<PaneManager>,
    socket_path: &Option<PathBuf>,
    editor_wrap: bool,
    preview_wrap: bool,
) {
    match node {
        PaneNode::Leaf(leaf) if leaf.id == target_id => {
            let new_leaf =
                create_empty_leaf(new_id, socket_path.clone(), editor_wrap, preview_wrap, cx);
            let resize_state = cx.new(|_| ResizableState::default());
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
            split_leaf_in_node(
                &mut children.0,
                target_id,
                direction,
                new_id,
                split_id,
                cx,
                socket_path,
                editor_wrap,
                preview_wrap,
            );
            split_leaf_in_node(
                &mut children.1,
                target_id,
                direction,
                new_id,
                split_id,
                cx,
                socket_path,
                editor_wrap,
                preview_wrap,
            );
        }
        _ => {}
    }
}

fn create_empty_leaf(
    id: PaneId,
    socket_path: Option<PathBuf>,
    editor_wrap: bool,
    preview_wrap: bool,
    cx: &mut Context<PaneManager>,
) -> PaneLeaf {
    let editor = cx.new(|cx| Editor::new(cx));
    if let Some(ref socket) = socket_path {
        editor.update(cx, |ed, _| ed.set_socket_path(socket.clone()));
    }
    editor.update(cx, |ed, _| ed.set_wrap(editor_wrap));
    let preview = cx.new(|cx| Preview::from_markdown("", None, cx));
    preview.update(cx, |p, _| p.set_wrap(preview_wrap));

    PaneLeaf {
        id,
        file_path: None,
        title: String::new(),
        editor,
        preview,
        view_mode: ViewMode::Editor,
    }
}

pub struct PaneManager {
    root: PaneNode,
    focused: PaneId,
    next_id: usize,
    focus_handle: FocusHandle,
    ui: UiColors,
    socket_path: Option<PathBuf>,
    editor_wrap: bool,
    preview_wrap: bool,
    _editor_subscriptions: Vec<Subscription>,
}

impl PaneManager {
    pub fn new(cx: &mut App) -> Self {
        let initial_id = PaneId(0);
        let editor = cx.new(|cx| Editor::new(cx));
        let preview = cx.new(|cx| Preview::from_markdown("", None, cx));

        Self {
            root: PaneNode::Leaf(PaneLeaf {
                id: initial_id,
                file_path: None,
                title: String::new(),
                editor,
                preview,
                view_mode: ViewMode::Editor,
            }),
            focused: initial_id,
            next_id: 1,
            focus_handle: cx.focus_handle(),
            ui: UiColors::default(),
            socket_path: None,
            editor_wrap: true,
            preview_wrap: true,
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
        self.for_each_leaf_mut(
            |leaf, cx| {
                leaf.editor.update(cx, |ed, _| ed.set_wrap(editor_wrap));
                leaf.preview.update(cx, |p, _| p.set_wrap(preview_wrap));
            },
            cx,
        );
    }

    fn for_each_leaf_mut(&mut self, f: impl Fn(&mut PaneLeaf, &mut App), cx: &mut App) {
        fn apply(node: &mut PaneNode, f: &impl Fn(&mut PaneLeaf, &mut App), cx: &mut App) {
            match node {
                PaneNode::Leaf(leaf) => f(leaf, cx),
                PaneNode::Split { children, .. } => {
                    apply(&mut children.0, f, cx);
                    apply(&mut children.1, f, cx);
                }
            }
        }
        apply(&mut self.root, &f, cx);
    }

    fn auto_save_focused(&self, cx: &mut Context<Self>) {
        if let Some(leaf) = self.root.find_leaf(self.focused) {
            if leaf.file_path.is_some() && leaf.editor.read(cx).is_dirty() {
                let editor = leaf.editor.clone();
                editor.update(cx, |ed, _| ed.save_to_disk());
            }
        }
    }

    pub fn open_in_focused(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // If already open, focus that leaf
        if let Some(leaf_id) = self.root.find_file(&path) {
            self.focused = leaf_id;
            cx.notify();
            return;
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

        // Observe editor for title and preview sync
        let editor_clone = editor.clone();
        let sub = cx.observe(&editor, move |this, editor, cx| {
            this.sync_editor_to_leaf(&editor, cx);
            cx.notify();
        });
        self._editor_subscriptions.push(sub);

        // Update focused leaf
        if let Some(leaf) = self.root.find_leaf_mut(self.focused) {
            leaf.file_path = Some(path);
            leaf.title = title;
            leaf.editor = editor_clone;
            leaf.preview = preview;
            leaf.view_mode = ViewMode::Editor;
        }

        cx.notify();
    }

    fn sync_editor_to_leaf(&mut self, editor: &Entity<Editor>, cx: &mut Context<Self>) {
        if let Some(leaf) = self.root.find_leaf_by_editor_mut(editor) {
            let new_title = editor.read(cx).title().to_string();
            if leaf.title != new_title {
                leaf.title = new_title;
            }
            let text = editor.read(cx).text().to_string();
            leaf.preview.update(cx, |p, _| p.update_content(&text));
        }
    }

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
        let focused = self.focused;
        let new_id = PaneId(self.next_id);
        self.next_id += 1;
        let split_id = PaneId(self.next_id);
        self.next_id += 1;

        let socket_path = &self.socket_path;
        let editor_wrap = self.editor_wrap;
        let preview_wrap = self.preview_wrap;

        split_leaf_in_node(
            &mut self.root,
            focused,
            direction,
            new_id,
            split_id,
            cx,
            socket_path,
            editor_wrap,
            preview_wrap,
        );
        self.focused = new_id;
        cx.notify();
    }

    pub fn handle_close_pane(
        &mut self,
        _: &ClosePane,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.root.is_single_leaf() {
            // Clear the last remaining pane
            if let PaneNode::Leaf(leaf) = &mut self.root {
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

        // Auto-save before closing
        self.auto_save_focused(cx);

        let focused = self.focused;
        close_leaf_in_node(&mut self.root, focused);
        self.focused = self.root.first_leaf_id();
        cx.notify();
    }

    pub fn handle_next_pane(&mut self, _: &NextPane, _window: &mut Window, cx: &mut Context<Self>) {
        let leaves = self.root.collect_leaves();
        if leaves.len() <= 1 {
            return;
        }
        self.auto_save_focused(cx);
        if let Some(pos) = leaves.iter().position(|&id| id == self.focused) {
            self.focused = leaves[(pos + 1) % leaves.len()];
        }
        cx.notify();
    }

    pub fn handle_prev_pane(&mut self, _: &PrevPane, _window: &mut Window, cx: &mut Context<Self>) {
        let leaves = self.root.collect_leaves();
        if leaves.len() <= 1 {
            return;
        }
        self.auto_save_focused(cx);
        if let Some(pos) = leaves.iter().position(|&id| id == self.focused) {
            self.focused = if pos == 0 {
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
        if let Some(leaf) = self.root.find_leaf_mut(self.focused) {
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

    #[allow(dead_code)]
    pub fn active_editor(&self) -> Option<&Entity<Editor>> {
        self.root.find_leaf(self.focused).map(|l| &l.editor)
    }

    pub fn active_editor_title(&self, cx: &App) -> (Option<PathBuf>, Option<String>) {
        if let Some(leaf) = self.root.find_leaf(self.focused) {
            let path = leaf.file_path.clone();
            let title = leaf.editor.read(cx).title().to_string();
            return (path, Some(title));
        }
        (None, None)
    }
}

impl Focusable for PaneManager {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn render_pane_node(
    node: &PaneNode,
    focused: PaneId,
    border: gpui::Hsla,
    text_dim: gpui::Hsla,
) -> gpui::AnyElement {
    match node {
        PaneNode::Leaf(leaf) => {
            let is_focused = leaf.id == focused;
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

            div()
                .flex()
                .flex_col()
                .flex_1()
                .when(is_focused, |el| el.border_1().border_color(border))
                .child(header)
                .child(content)
                .into_any_element()
        }
        PaneNode::Split {
            id,
            direction,
            children,
            resize_state,
        } => {
            let left = render_pane_node(&children.0, focused, border, text_dim);
            let right = render_pane_node(&children.1, focused, border, text_dim);

            let group_id = ("pane-split", id.0);

            match direction {
                SplitDirection::Horizontal => h_resizable(group_id)
                    .with_state(resize_state)
                    .child(resizable_panel().child(left))
                    .child(resizable_panel().child(right))
                    .into_any_element(),
                SplitDirection::Vertical => v_resizable(group_id)
                    .with_state(resize_state)
                    .child(resizable_panel().child(left))
                    .child(resizable_panel().child(right))
                    .into_any_element(),
            }
        }
    }
}

impl Render for PaneManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border = parse_hex(&self.ui.border);
        let text_dim = parse_hex(&self.ui.text_dim);
        let focused = self.focused;

        // Single leaf: render header + content as direct children (no wrapper)
        // Split: use recursive rendering
        let mut main = div()
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus_handle);

        match &self.root {
            PaneNode::Leaf(leaf) => {
                // Header
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
                main = main.child(header);

                // Content
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
                main = main.child(content);
            }
            PaneNode::Split { .. } => {
                let tree = render_pane_node(&self.root, focused, border, text_dim);
                main = main.child(tree);
            }
        }

        // Focus the active view
        if let Some(leaf) = self.root.find_leaf(focused) {
            match leaf.view_mode {
                ViewMode::Editor => leaf.editor.focus_handle(cx).focus(window),
                ViewMode::Preview => leaf.preview.focus_handle(cx).focus(window),
                ViewMode::Split => leaf.editor.focus_handle(cx).focus(window),
            }
        } else {
            self.focus_handle.focus(window);
        }

        main.on_action(cx.listener(PaneManager::handle_split_right))
            .on_action(cx.listener(PaneManager::handle_split_down))
            .on_action(cx.listener(PaneManager::handle_close_pane))
            .on_action(cx.listener(PaneManager::handle_next_pane))
            .on_action(cx.listener(PaneManager::handle_prev_pane))
            .on_action(cx.listener(PaneManager::handle_toggle_view))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_id_equality() {
        assert_eq!(PaneId(0), PaneId(0));
        assert_ne!(PaneId(0), PaneId(1));
    }

    #[test]
    fn split_direction_values() {
        assert_ne!(SplitDirection::Horizontal, SplitDirection::Vertical);
    }

    #[test]
    fn view_mode_cycle() {
        let mode = ViewMode::Editor;
        assert_eq!(
            match mode {
                ViewMode::Editor => ViewMode::Split,
                ViewMode::Split => ViewMode::Preview,
                ViewMode::Preview => ViewMode::Editor,
            },
            ViewMode::Split
        );
    }
}
