# markdown

## Role

A crate that parses Markdown strings into a custom AST (Abstract Syntax Tree). No external dependencies.

## Module Layout

```
src/
├── lib.rs      Public API (parse function), re-exports
├── ast.rs      AST struct definitions (MarkdownDoc, Block, Inline, ListItem, etc.)
├── block.rs    Block detection (BlockKind, BlockSlice, detect_blocks)
└── parser.rs   Parser core, frontmatter splitting, block construction
```

## Dependencies

None (standard library only)

## Key Types / APIs

### MarkdownDoc (ast.rs)

Root struct of the parse result.

```rust
struct MarkdownDoc {
    frontmatter: Option<String>,  // Raw YAML frontmatter string (unparsed)
    blocks: Vec<Block>,           // List of block elements
}
```

### Block enum (ast.rs)

Block-level elements. 10 variants.

```rust
enum Block {
    Heading { level: u8, children: Vec<Inline> },
    Paragraph(Vec<Inline>),
    CodeBlock { language: Option<String>, code: String },
    List { items: Vec<ListItem> },
    BlockQuote(Vec<Block>),                            // Recursive
    Table { headers, aligns, rows },
    ThematicBreak,
    MathBlock { content: String },
    HtmlBlock { content: String },
    FootnoteDefinition { label: String, content: Vec<Block> },  // Recursive
}
```

### Inline enum (ast.rs)

Inline elements. 13 variants.

```rust
enum Inline {
    Text(String),
    Bold(Vec<Inline>),              // Nestable
    Italic(Vec<Inline>),            // Nestable
    Strikethrough(Vec<Inline>),     // Nestable
    Code(String),                   // `code`
    Link { text: Vec<Inline>, url: String, title: Option<String> },
    Image { alt: String, url: String, title: Option<String> },
    Math(String),                   // $math$
    FootnoteRef(String),            // [^label]
    HtmlTag(String),
    HardBreak,
    SoftBreak,
}
```

### ListItem struct (ast.rs)

A list item with recursive sub-items.

```rust
struct ListItem {
    marker: ListMarker,
    children: Vec<Inline>,
    sub_items: Vec<ListItem>,     // Nested list
}
```

### ListMarker enum (ast.rs)

```rust
enum ListMarker {
    Dash,       // -
    Plus,       // +
    Star,       // *
    Number(u32), // 1.
}
```

### TableAlign enum (ast.rs)

```rust
enum TableAlign {
    Left,    // :---
    Center,  // :---:
    Right,   // ---:
}
```

### parse() function (parser.rs)

Parser entry point.

```rust
pub fn parse(input: &str) -> MarkdownDoc
```

**Parse pipeline:**

1. `split_frontmatter(input)` — Separate YAML frontmatter delimited by `---`
2. `block::detect_blocks(&lines)` — Detect `BlockSlice` array from line list
3. Call `build_block()` for each slice to construct a Block
   - Inline elements are parsed recursively via `inline::parse_inline(text)`

**BlockQuote / FootnoteDefinition** support nesting by recursively passing their content to `parse()`.

### Table parser

- Line 1: Header (`|`-delimited)
- Line 2: Separator (`:---:` etc. → TableAlign)
- Line 3+: Data rows

### List parser

- Auto-detects marker type (`-`, `+`, `*`, numbers)
- Recursively collects indented sub-items
- Includes continuation lines (non-empty non-marker lines) in the same item

## Data Flow

```
"# Title\n\nHello **bold**\n\n```rust\ncode\n```"
    │
    parse()
    ├── split_frontmatter()
    │     → (None, "# Title\n\nHello **bold**\n\n```rust\ncode\n```")
    │
    ├── Collect lines → ["# Title", "", "Hello **bold**", "", "```rust", "code", "```"]
    │
    ├── detect_blocks()
    │     → [Heading{1}, Paragraph, CodeBlock{rust}]
    │
    └── build_block() for each:
          ├─ Heading → inline::parse_inline("Title") → [Text("Title")]
          ├─ Paragraph → inline::parse_inline("Hello **bold**")
          │     → [Text("Hello "), Bold([Text("bold")])]
          └─ CodeBlock → language: Some("rust"), code: "code"

Result:
MarkdownDoc {
    frontmatter: None,
    blocks: [
        Heading { level: 1, children: [Text("Title")] },
        Paragraph([Text("Hello "), Bold([Text("bold")])]),
        CodeBlock { language: Some("rust"), code: "code" },
    ]
}
```
