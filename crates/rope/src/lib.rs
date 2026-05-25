/// B-tree based Rope for efficient text editing.
/// Leaf nodes hold up to CHUNK_SIZE bytes.
/// Internal nodes hold 2 children + left subtree metrics.

const CHUNK_SIZE: usize = 512;
const MIN_SPLIT: usize = CHUNK_SIZE / 4;

#[derive(Debug, Clone)]
pub enum Node {
    Leaf {
        text: String,
        line_count: usize,
    },
    Internal {
        left: Box<Node>,
        right: Box<Node>,
        char_count: usize,
        line_count: usize,
    },
}

impl Node {
    pub fn char_count(&self) -> usize {
        match self {
            Node::Leaf { text, .. } => text.len(),
            Node::Internal { char_count, .. } => *char_count,
        }
    }

    pub fn line_count(&self) -> usize {
        match self {
            Node::Leaf { line_count, .. } => *line_count,
            Node::Internal { line_count, .. } => *line_count,
        }
    }

    pub fn from_str(s: &str) -> Self {
        if s.len() <= CHUNK_SIZE {
            let line_count = s.lines().count().max(1);
            Node::Leaf { text: s.to_string(), line_count }
        } else {
            let mid = find_split_point(s);
            let left = Node::from_str(&s[..mid]);
            let right = Node::from_str(&s[mid..]);
            Node::merge(left, right)
        }
    }

    fn merge(left: Node, right: Node) -> Self {
        let char_count = left.char_count() + right.char_count();
        let line_count = left.line_count() + right.line_count();
        Node::Internal {
            left: Box::new(left),
            right: Box::new(right),
            char_count,
            line_count,
        }
    }

    pub fn insert(&self, pos: usize, text: &str) -> Self {
        if text.is_empty() { return self.clone(); }

        match self {
            Node::Leaf { text: leaf_text, .. } => {
                let mut new_text = leaf_text.clone();
                if pos > new_text.len() {
                    new_text.push_str(text);
                } else {
                    new_text.insert_str(pos, text);
                }
                if new_text.len() <= CHUNK_SIZE {
                    let line_count = new_text.lines().count().max(1);
                    Node::Leaf { text: new_text, line_count }
                } else {
                    Node::from_str(&new_text)
                }
            }
            Node::Internal { left, right, char_count, .. } => {
                let left_len = left.char_count();
                if pos <= left_len {
                    let new_left = left.insert(pos, text);
                    Node::merge(new_left, *right.clone())
                } else {
                    let new_right = right.insert(pos - left_len, text);
                    Node::merge(*left.clone(), new_right)
                }
            }
        }
    }

    pub fn delete(&self, start: usize, end: usize) -> Self {
        if start >= end { return self.clone(); }

        match self {
            Node::Leaf { text: leaf_text, .. } => {
                let s = start.min(leaf_text.len());
                let e = end.min(leaf_text.len());
                let mut new_text = leaf_text.clone();
                new_text.replace_range(s..e, "");
                if new_text.is_empty() {
                    Node::Leaf { text: new_text, line_count: 1 }
                } else {
                    let line_count = new_text.lines().count().max(1);
                    Node::Leaf { text: new_text, line_count }
                }
            }
            Node::Internal { left, right, .. } => {
                let left_len = left.char_count();

                if end <= left_len {
                    let new_left = left.delete(start, end);
                    Self::rebalance(new_left, *right.clone())
                } else if start >= left_len {
                    let new_right = right.delete(start - left_len, end - left_len);
                    Self::rebalance(*left.clone(), new_right)
                } else {
                    // spans both children
                    let new_left = left.delete(start, left_len);
                    let new_right = right.delete(0, end - left_len);
                    Self::rebalance(new_left, new_right)
                }
            }
        }
    }

    fn rebalance(left: Node, right: Node) -> Node {
        // if one side is empty, return the other
        if left.char_count() == 0 { return right; }
        if right.char_count() == 0 { return left; }
        Node::merge(left, right)
    }

    /// Get the text content of line `idx` (0-indexed), without newline.
    pub fn line(&self, idx: usize) -> String {
        let mut current_line = 0;
        let mut result = String::new();
        self.collect_line(idx, &mut current_line, &mut result);
        result
    }

    fn collect_line(&self, target: usize, current_line: &mut usize, result: &mut String) -> bool {
        match self {
            Node::Leaf { text, .. } => {
                for (i, line) in text.lines().enumerate() {
                    if *current_line + i == target {
                        result.push_str(line);
                        return true;
                    }
                }
                // Handle trailing content without newline
                if let Some(last_newline) = text.rfind('\n') {
                    let after = &text[last_newline + 1..];
                    if !after.is_empty() && *current_line + text.lines().count() - 1 == target {
                        // already handled above
                    }
                }
                *current_line += text.lines().count();
                if text.ends_with('\n') {
                    // empty line after trailing newline
                }
                false
            }
            Node::Internal { left, right, .. } => {
                if left.collect_line(target, current_line, result) {
                    return true;
                }
                right.collect_line(target, current_line, result)
            }
        }
    }

    /// Get full text content.
    pub fn text(&self) -> String {
        match self {
            Node::Leaf { text, .. } => text.clone(),
            Node::Internal { left, right, .. } => {
                let mut s = left.text();
                s.push_str(&right.text());
                s
            }
        }
    }

    /// Get character at position.
    pub fn char_at(&self, pos: usize) -> Option<char> {
        match self {
            Node::Leaf { text, .. } => text.chars().nth(pos),
            Node::Internal { left, right, .. } => {
                let left_len = left.char_count();
                if pos < left_len {
                    left.char_at(pos)
                } else {
                    right.char_at(pos - left_len)
                }
            }
        }
    }
}

fn find_split_point(s: &str) -> usize {
    let mid = s.len() / 2;
    // find nearest newline or space
    for i in 0..s.len() / 4 {
        if mid + i < s.len() && s.as_bytes()[mid + i] == b'\n' {
            return mid + i + 1;
        }
        if mid >= i && s.as_bytes()[mid - i] == b'\n' {
            return mid - i + 1;
        }
    }
    mid
}

/// A Rope-based text buffer.
pub struct Rope {
    root: Node,
}

impl Rope {
    pub fn new() -> Self {
        Self {
            root: Node::Leaf { text: String::new(), line_count: 1 },
        }
    }

    pub fn from(text: &str) -> Self {
        if text.is_empty() {
            Self::new()
        } else {
            Self { root: Node::from_str(text) }
        }
    }

    pub fn char_count(&self) -> usize {
        self.root.char_count()
    }

    pub fn line_count(&self) -> usize {
        self.root.line_count()
    }

    pub fn insert(&mut self, pos: usize, text: &str) {
        self.root = self.root.insert(pos, text);
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        self.root = self.root.delete(start, end);
    }

    pub fn line(&self, idx: usize) -> String {
        self.root.line(idx)
    }

    pub fn text(&self) -> String {
        self.root.text()
    }

    pub fn char_at(&self, pos: usize) -> Option<char> {
        self.root.char_at(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rope() {
        let rope = Rope::new();
        assert_eq!(rope.text(), "");
        assert_eq!(rope.char_count(), 0);
    }

    #[test]
    fn from_text() {
        let rope = Rope::from("hello world");
        assert_eq!(rope.text(), "hello world");
        assert_eq!(rope.char_count(), 11);
    }

    #[test]
    fn insert_at_start() {
        let mut rope = Rope::from("world");
        rope.insert(0, "hello ");
        assert_eq!(rope.text(), "hello world");
    }

    #[test]
    fn insert_at_end() {
        let mut rope = Rope::from("hello");
        rope.insert(5, " world");
        assert_eq!(rope.text(), "hello world");
    }

    #[test]
    fn insert_in_middle() {
        let mut rope = Rope::from("hello world");
        rope.insert(5, " beautiful");
        assert_eq!(rope.text(), "hello beautiful world");
    }

    #[test]
    fn delete_from_start() {
        let mut rope = Rope::from("hello world");
        rope.delete(0, 6);
        assert_eq!(rope.text(), "world");
    }

    #[test]
    fn delete_from_middle() {
        let mut rope = Rope::from("hello beautiful world");
        rope.delete(5, 15);
        assert_eq!(rope.text(), "hello world");
    }

    #[test]
    fn line_access() {
        let rope = Rope::from("line one\nline two\nline three");
        assert_eq!(rope.line(0), "line one");
        assert_eq!(rope.line(1), "line two");
        assert_eq!(rope.line(2), "line three");
    }

    #[test]
    fn large_text_insert() {
        let mut rope = Rope::new();
        for i in 0..100 {
            rope.insert(rope.char_count(), &format!("line {i}\n"));
        }
        assert!(rope.text().contains("line 99"));
        assert!(rope.line_count() >= 100);
    }

    #[test]
    fn char_at() {
        let rope = Rope::from("hello");
        assert_eq!(rope.char_at(0), Some('h'));
        assert_eq!(rope.char_at(4), Some('o'));
        assert_eq!(rope.char_at(5), None);
    }

    #[test]
    fn insert_causes_split() {
        let long_text = "a".repeat(600);
        let mut rope = Rope::from(&long_text);
        rope.insert(300, "INSERTED");
        let text = rope.text();
        assert!(text.contains("INSERTED"));
        assert_eq!(text.len(), 608);
    }
}

// --- Buffer with undo/redo ---

#[derive(Debug, Clone)]
enum Edit {
    Insert { pos: usize, text: String },
    Delete { start: usize, end: usize, text: String },
}

pub struct Buffer {
    rope: Rope,
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn from(text: &str) -> Self {
        Self {
            rope: Rope::from(text),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn edit(&mut self, start: usize, end: usize, new_text: &str) {
        let deleted = self.rope.text()[start..end].to_string();
        self.rope.delete(start, end);
        if !new_text.is_empty() {
            self.rope.insert(start, new_text);
        }
        self.undo_stack.push(Edit::Insert { pos: start, text: new_text.to_string() });
        self.undo_stack.push(Edit::Delete { start, end, text: deleted });
        self.redo_stack.clear();
    }

    pub fn insert(&mut self, pos: usize, text: &str) {
        self.edit(pos, pos, text);
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        self.edit(start, end, "");
    }

    pub fn undo(&mut self) -> bool {
        let Some(delete_edit) = self.undo_stack.pop() else { return false };
        let Some(insert_edit) = self.undo_stack.pop() else { return false };

        match (insert_edit, delete_edit) {
            (Edit::Insert { pos, text: inserted }, Edit::Delete { text: deleted, .. }) => {
                if !inserted.is_empty() {
                    self.rope.delete(pos, pos + inserted.len());
                }
                if !deleted.is_empty() {
                    self.rope.insert(pos, &deleted);
                }
                self.redo_stack.push(Edit::Insert { pos, text: inserted });
                self.redo_stack.push(Edit::Delete { start: pos, end: pos + deleted.len(), text: deleted });
                true
            }
            _ => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        let Some(delete_edit) = self.redo_stack.pop() else { return false };
        let Some(insert_edit) = self.redo_stack.pop() else { return false };

        match (insert_edit, delete_edit) {
            (Edit::Insert { pos, text: inserted }, Edit::Delete { text: deleted, .. }) => {
                if !deleted.is_empty() {
                    self.rope.delete(pos, pos + deleted.len());
                }
                if !inserted.is_empty() {
                    self.rope.insert(pos, &inserted);
                }
                self.undo_stack.push(Edit::Insert { pos, text: inserted });
                self.undo_stack.push(Edit::Delete { start: pos, end: pos + deleted.len(), text: deleted });
                true
            }
            _ => false,
        }
    }

    pub fn text(&self) -> String {
        self.rope.text()
    }

    pub fn line(&self, idx: usize) -> String {
        self.rope.line(idx)
    }

    pub fn line_count(&self) -> usize {
        self.rope.line_count()
    }

    pub fn char_count(&self) -> usize {
        self.rope.char_count()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    #[test]
    fn buffer_insert_and_text() {
        let mut buf = Buffer::new();
        buf.insert(0, "hello");
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn buffer_delete() {
        let mut buf = Buffer::from("hello world");
        buf.delete(5, 11);
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn buffer_undo_insert() {
        let mut buf = Buffer::from("world");
        buf.insert(0, "hello ");
        assert_eq!(buf.text(), "hello world");
        assert!(buf.undo());
        assert_eq!(buf.text(), "world");
    }

    #[test]
    fn buffer_undo_delete() {
        let mut buf = Buffer::from("hello world");
        buf.delete(5, 11);
        assert_eq!(buf.text(), "hello");
        assert!(buf.undo());
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn buffer_redo() {
        let mut buf = Buffer::from("hello");
        buf.insert(5, " world");
        buf.undo();
        assert_eq!(buf.text(), "hello");
        assert!(buf.redo());
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn buffer_multiple_undo_redo() {
        let mut buf = Buffer::new();
        buf.insert(0, "a");
        buf.insert(1, "b");
        buf.insert(2, "c");
        assert_eq!(buf.text(), "abc");

        buf.undo();
        assert_eq!(buf.text(), "ab");
        buf.undo();
        assert_eq!(buf.text(), "a");
        buf.undo();
        assert_eq!(buf.text(), "");

        buf.redo();
        assert_eq!(buf.text(), "a");
        buf.redo();
        assert_eq!(buf.text(), "ab");
    }

    #[test]
    fn buffer_redo_cleared_on_new_edit() {
        let mut buf = Buffer::from("hello");
        buf.insert(5, " world");
        buf.undo();
        assert!(buf.can_redo());
        buf.insert(5, "!");
        assert!(!buf.can_redo());
    }
}
