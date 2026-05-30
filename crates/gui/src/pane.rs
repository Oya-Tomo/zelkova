use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div, px,
};
use gpui_component::resizable::{ResizableState, h_resizable, resizable_panel, v_resizable};

use crate::editor::Editor;
use crate::preview::Preview;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Editor,
    SplitHorizontal,
    SplitVertical,
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
    pub resize_state: Entity<ResizableState>,
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
    pub fn find_leaf(&self, id: PaneId) -> Option<&PaneLeaf> {
        match self {
            PaneNode::Leaf(leaf) if leaf.id == id => Some(leaf),
            PaneNode::Leaf(_) => None,
            PaneNode::Split { children, .. } => children
                .0
                .find_leaf(id)
                .or_else(|| children.1.find_leaf(id)),
        }
    }

    pub fn find_leaf_mut(&mut self, id: PaneId) -> Option<&mut PaneLeaf> {
        match self {
            PaneNode::Leaf(leaf) if leaf.id == id => Some(leaf),
            PaneNode::Leaf(_) => None,
            PaneNode::Split { children, .. } => children
                .0
                .find_leaf_mut(id)
                .or_else(|| children.1.find_leaf_mut(id)),
        }
    }

    pub fn collect_leaves(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf(leaf) => vec![leaf.id],
            PaneNode::Split { children, .. } => {
                let mut leaves = children.0.collect_leaves();
                leaves.extend(children.1.collect_leaves());
                leaves
            }
        }
    }

    pub fn find_file(&self, path: &PathBuf) -> Option<PaneId> {
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

    pub fn find_leaf_by_editor_mut(&mut self, editor: &Entity<Editor>) -> Option<&mut PaneLeaf> {
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

    pub fn first_leaf_id(&self) -> PaneId {
        match self {
            PaneNode::Leaf(leaf) => leaf.id,
            PaneNode::Split { children, .. } => children.0.first_leaf_id(),
        }
    }

    pub fn is_single_leaf(&self) -> bool {
        matches!(self, PaneNode::Leaf(_))
    }
}

pub fn close_leaf_in_node(node: &mut PaneNode, target_id: PaneId) -> bool {
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

pub fn create_empty_leaf(
    id: PaneId,
    socket_path: Option<PathBuf>,
    editor_wrap: bool,
    preview_wrap: bool,
    cx: &mut App,
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
        resize_state: cx.new(|_| ResizableState::default()),
    }
}

pub fn render_pane_node(
    node: &PaneNode,
    focused: PaneId,
    border: gpui::Hsla,
    text_dim: gpui::Hsla,
    on_focus: Rc<dyn Fn(PaneId, &mut App)>,
) -> gpui::AnyElement {
    match node {
        PaneNode::Leaf(leaf) => {
            let is_focused = leaf.id == focused;
            let leaf_id = leaf.id;
            let on_focus_clone = on_focus.clone();

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
                    ViewMode::SplitHorizontal => {
                        let split = h_resizable(("editor-preview-split", leaf_id.0))
                            .with_state(&leaf.resize_state)
                            .child(
                                resizable_panel().child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .child(leaf.editor.clone()),
                                ),
                            )
                            .child(
                                resizable_panel().child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .child(leaf.preview.clone()),
                                ),
                            );
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .child(split)
                            .into_any_element()
                    }
                    ViewMode::SplitVertical => {
                        let split = v_resizable(("editor-preview-split", leaf_id.0))
                            .with_state(&leaf.resize_state)
                            .child(
                                resizable_panel().child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .child(leaf.editor.clone()),
                                ),
                            )
                            .child(
                                resizable_panel().child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .overflow_hidden()
                                        .child(leaf.preview.clone()),
                                ),
                            );
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .child(split)
                            .into_any_element()
                    }
                }
            };

            div()
                .flex()
                .flex_col()
                .flex_1()
                .when(is_focused, |el| el.border_1().border_color(border))
                .on_mouse_down(gpui::MouseButton::Left, move |_event, _window, cx| {
                    on_focus_clone(leaf_id, cx);
                })
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
            let left = render_pane_node(&children.0, focused, border, text_dim, on_focus.clone());
            let right = render_pane_node(&children.1, focused, border, text_dim, on_focus);

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
                ViewMode::Editor => ViewMode::SplitHorizontal,
                ViewMode::SplitHorizontal => ViewMode::SplitVertical,
                ViewMode::SplitVertical => ViewMode::Preview,
                ViewMode::Preview => ViewMode::Editor,
            },
            ViewMode::SplitHorizontal
        );
    }
}
