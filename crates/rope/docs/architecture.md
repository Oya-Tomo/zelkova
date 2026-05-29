# rope

## Role

A crate providing a B-tree-based Rope data structure for efficient text editing, along with a text buffer with undo/redo support. No external dependencies.

## Module Layout

```
src/
└── lib.rs    All implementations (Node, Rope, Buffer) ~545 lines
```

## Dependencies

None (standard library only)

## Key Types / APIs

### Constants

| Constant | Value | Purpose |
|---|---|---|
| `CHUNK_SIZE` | 512 | Maximum bytes per leaf node |
| `MIN_SPLIT` | 128 (`CHUNK_SIZE / 4`) | Minimum split threshold (currently unused) |

### Node enum

Internal node of the Rope. Immutable as a persistent data structure (insert/delete return new Nodes).

```rust
enum Node {
    Leaf {
        text: String,         // Text fragment up to CHUNK_SIZE bytes
        line_count: usize,    // Line count based on newline count (minimum 1)
    },
    Internal {
        left: Box<Node>,      // Left subtree
        right: Box<Node>,     // Right subtree
        char_count: usize,    // Total byte count of left + right
        line_count: usize,    // Total line count of left + right
    },
}
```

**Methods:**

| Method | Signature | Complexity | Description |
|---|---|---|---|
| `char_count` | `&self -> usize` | O(1) | Return byte count (leaf: text.len, internal: cached) |
| `line_count` | `&self -> usize` | O(1) | Return line count |
| `from_str` | `&str -> Self` | O(n) | Build from string. Splits recursively if exceeding CHUNK_SIZE |
| `insert` | `&self, usize, &str -> Self` | O(log n) | Insert text at specified position |
| `delete` | `&self, usize, usize -> Self` | O(log n) | Delete range [start, end) |
| `line` | `&self, usize -> String` | O(n) worst | Get text at specified line |
| `text` | `&self -> String` | O(n) | Concatenate and return all text |
| `char_at` | `&self, usize -> Option<char>` | O(log n) | Get character at specified position |

### Split Point Determination (find_split_point)

When building a string that exceeds `CHUNK_SIZE`, searches for a newline or space near the midpoint to split on. Falls back to splitting at the midpoint if none is found.

### Rebalancing (rebalance)

After deletion, if one child is empty, returns the other child directly. Otherwise, merges both via `merge`.

### Rope struct

Public API wrapping the Node root.

```rust
struct Rope {
    root: Node,
}
```

| Method | Description |
|---|---|
| `new()` | Create an empty Rope |
| `from(text)` | Create a Rope from a string |
| `char_count()` | Byte count |
| `line_count()` | Line count |
| `insert(pos, text)` | Insert text at position pos |
| `delete(start, end)` | Delete range |
| `line(idx)` | Get content of line idx |
| `text()` | Get all text |
| `char_at(pos)` | Get character at position pos |

### Buffer struct

A text buffer that wraps a Rope and manages undo/redo stacks.

```rust
struct Buffer {
    rope: Rope,
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
}
```

#### Edit enum (private)

```rust
enum Edit {
    Insert { pos: usize, text: String },
    Delete { start: usize, end: usize, text: String },
}
```

A single edit operation is always recorded as an `[Insert, Delete]` pair on the undo_stack:
- `edit(start, end, new_text)` → pushes Delete(old text) + Insert(new text)
- `undo()` → Applies the pair in reverse order (delete Insert, restore Delete)
- `redo()` → Re-applies the pair

**Methods:**

| Method | Description |
|---|---|
| `new()` | Create an empty buffer |
| `from(text)` | Create a buffer with initial text |
| `edit(start, end, new_text)` | Replace range (with undo/redo recording) |
| `insert(pos, text)` | Shorthand for `edit(pos, pos, text)` |
| `delete(start, end)` | Shorthand for `edit(start, end, "")` |
| `undo() -> bool` | Undo the last edit |
| `redo() -> bool` | Redo a previously undone edit |
| `text()` | Get all text |
| `line(idx)` | Get content of line idx |
| `line_count()` | Line count |
| `char_count()` | Byte count |
| `can_undo()` | Whether undo_stack is non-empty |
| `can_redo()` | Whether redo_stack is non-empty |

## Data Flow

```
Text insertion:
  Buffer::edit(start, end, "new")
    ├── rope.delete(start, end)        → New Node tree
    ├── rope.insert(start, "new")      → New Node tree
    ├── undo_stack.push(Insert{...})
    ├── undo_stack.push(Delete{...})
    └── redo_stack.clear()

undo:
  Pop [Insert, Delete] pair from undo_stack
    ├── Delete text from Insert via rope.delete()
    ├── Restore text from Delete via rope.insert()
    └── Push pair to redo_stack

redo:
  Pop [Insert, Delete] pair from redo_stack
    ├── Delete text from Delete via rope.delete()
    ├── Re-insert text from Insert via rope.insert()
    └── Push pair to undo_stack
```

### Rope Internal Structure Example

```
"Hello World\nFoo\nBar" → Under CHUNK_SIZE=512, single leaf:

Node::Leaf { text: "Hello World\nFoo\nBar", line_count: 3 }

Large text (>512 bytes):
          Internal { char_count: 1200, line_count: 30 }
         /                                              \
  Leaf { text: "...512B...", line_count: 15 }    Internal { char_count: 688, line_count: 15 }
                                                      /                                  \
                                              Leaf { "...512B..." }              Leaf { "...176B..." }
```
