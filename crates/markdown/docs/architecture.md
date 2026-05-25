# markdown

## Role

Markdown文字列を独自AST (抽象構文木) にパースするcrate。外部依存なし。

## Module Layout

```
src/
├── lib.rs      公開API (parse関数), re-export
├── ast.rs      AST構造体定義 (MarkdownDoc, Block, Inline, ListItem等)
├── block.rs    ブロック検出 (BlockKind, BlockSlice, detect_blocks)
└── parser.rs   パーサー本体, フロントマター分割, 各ブロックの構築
```

## Dependencies

なし (標準ライブラリのみ)

## Key Types / APIs

### MarkdownDoc (ast.rs)

パース結果のルート構造体。

```rust
struct MarkdownDoc {
    frontmatter: Option<String>,  // YAML frontmatterの生文字列 (未パース)
    blocks: Vec<Block>,           // ブロック要素のリスト
}
```

### Block enum (ast.rs)

ブロックレベル要素。10種類。

```rust
enum Block {
    Heading { level: u8, children: Vec<Inline> },
    Paragraph(Vec<Inline>),
    CodeBlock { language: Option<String>, code: String },
    List { items: Vec<ListItem> },
    BlockQuote(Vec<Block>),                            // 再帰的
    Table { headers, aligns, rows },
    ThematicBreak,
    MathBlock { content: String },
    HtmlBlock { content: String },
    FootnoteDefinition { label: String, content: Vec<Block> },  // 再帰的
}
```

### Inline enum (ast.rs)

インライン要素。13種類。

```rust
enum Inline {
    Text(String),
    Bold(Vec<Inline>),              // 入れ子可
    Italic(Vec<Inline>),            // 入れ子可
    Strikethrough(Vec<Inline>),     // 入れ子可
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

リスト項目。再帰的なサブアイテムを持つ。

```rust
struct ListItem {
    marker: ListMarker,
    children: Vec<Inline>,
    sub_items: Vec<ListItem>,     // ネストされたリスト
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

### parse() 関数 (parser.rs)

パーサーのエントリポイント。

```rust
pub fn parse(input: &str) -> MarkdownDoc
```

**パースパイプライン:**

1. `split_frontmatter(input)` — `---`区切りのYAML frontmatterを分離
2. `block::detect_blocks(&lines)` — 行リストから`BlockSlice`配列を検出
3. 各スライスに対して `build_block()` を呼び出しBlockを構築
   - `inline::parse_inline(text)` でインライン要素を再帰的にパース

**BlockQuote / FootnoteDefinition** は内容を`parse()`に再帰的に渡すことでネスト構造をサポート。

### テーブルパーサー

- 1行目: ヘッダー (`|`区切り)
- 2行目: セパレーター (`:---:`等 → TableAlign)
- 3行目以降: データ行

### リストパーサー

- マーカー種別を自動検出 (`-`, `+`, `*`, 数字)
- インデントされたサブアイテムを再帰的に収集
- 継続行 (空行でない非マーカー行) を同じアイテムに含める

## Data Flow

```
"# Title\n\nHello **bold**\n\n```rust\ncode\n```"
    │
    parse()
    ├── split_frontmatter()
    │     → (None, "# Title\n\nHello **bold**\n\n```rust\ncode\n```")
    │
    ├── lines收集 → ["# Title", "", "Hello **bold**", "", "```rust", "code", "```"]
    │
    ├── detect_blocks()
    │     → [Heading{1}, Paragraph, CodeBlock{rust}]
    │
    └── build_block() for each:
          ├─ Heading → inline::parse_inline("Title") → [Text("Title")]
          ├─ Paragraph → inline::parse_inline("Hello **bold**")
          │     → [Text("Hello "), Bold([Text("bold")])]
          └─ CodeBlock → language: Some("rust"), code: "code"

結果:
MarkdownDoc {
    frontmatter: None,
    blocks: [
        Heading { level: 1, children: [Text("Title")] },
        Paragraph([Text("Hello "), Bold([Text("bold")])]),
        CodeBlock { language: Some("rust"), code: "code" },
    ]
}
```
