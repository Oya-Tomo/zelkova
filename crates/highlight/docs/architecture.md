# highlight

## Role

Tree-sitter-based code block syntax highlighting. Parses source code inside Markdown code fences in the GUI editor by language and returns styled ranges (`StyledRange`).

This crate does not depend on GPUI. Color resolution (string-to-Hsla conversion) is handled by `ResolvedColors` on the GUI side.

## Module Layout

```
src/
├── lib.rs     Lazy initialization of language configs, resolve_language, highlight_code
└── theme.rs   CodeTheme, HIGHLIGHT_NAMES, color mapping definitions
```

## Supported Languages

| Language | Grammar Crate | Config Key | Query Constant |
|---|---|---|---|
| Rust | tree-sitter-rust 0.24 | `"rust"` | `HIGHLIGHTS_QUERY` |
| JS/TS | tree-sitter-javascript 0.25 | `"javascript"` | `HIGHLIGHT_QUERY` |
| Python | tree-sitter-python 0.25 | `"python"` | `HIGHLIGHTS_QUERY` |
| Go | tree-sitter-go 0.25 | `"go"` | `HIGHLIGHTS_QUERY` |
| C | tree-sitter-c 0.24 | `"c"` | `HIGHLIGHT_QUERY` |

Note: Grammar crate versions vary, but all depend on `tree-sitter-language = "0.1"`, making them compatible with `tree-sitter = "0.26"`.

## Key APIs

### `resolve_language(info: &str) -> Option<&'static str>`

Converts a Markdown fence info string to an internal language key.

```
"rust" | "rs"           → "rust"
"javascript" | "js"     → "javascript"
"typescript" | "ts"     → "javascript"  // TS highlighted with JS grammar
"python" | "py"         → "python"
"go" | "golang"         → "go"
"c" | "h"               → "c"
```

### `highlight_code(code: &str, language: &str) -> Vec<StyledRange>`

Parses the code string with Tree-sitter and returns a list of `StyledRange`. Each `StyledRange` holds a byte range and a highlight index. Returns an empty Vec for unknown languages or empty strings.

### `StyledRange`

```rust
pub struct StyledRange {
    pub range: Range<usize>,       // Byte offset range
    pub highlight_index: usize,    // Index into HIGHLIGHT_NAMES
}
```

### `HIGHLIGHT_NAMES`

12 highlight class names passed to Tree-sitter. Indices correspond to `Highlight(usize)`.

```rust
["attribute", "comment", "constant", "function", "keyword",
 "number", "operator", "property", "punctuation", "string", "tag", "type"]
```

### `CodeTheme`

Holds color strings (`#RRGGBB`) for the 12 classes. Since the GUI's `ResolvedColors` provides equivalent functionality with Hsla values, this struct is not used for direct rendering. It exists for testing and to maintain crate independence.

- `CodeTheme::from_editor_colors(&EditorColors)` — Construct from configuration
- `color_by_index(usize) -> Option<&str>` — Look up color string by index

## Internal Architecture

### Lazy Configuration (`CONFIGS`)

`HighlightConfiguration` initialization is expensive, so it is lazily evaluated via `Lazy<HashMap>`. Note the query constant names per grammar:

- Rust, Python, Go: `HIGHLIGHTS_QUERY` (plural)
- JavaScript, C: `HIGHLIGHT_QUERY` (singular)

### Data Flow

```
GUI: build_highlights() detects fence language
  │
  ├─ resolve_language("rust") → Some("rust")
  │
  ├─ highlight_code(source, "rust")
  │   ├─ CONFIGS.get("rust") → &HighlightConfiguration
  │   ├─ TsHighlighter::highlight() → HighlightEvent stream
  │   └─ Fold events into Vec<StyledRange>
  │
  └─ GUI: ResolvedColors.syntax_color(index) → Apply Hsla directly
```

## Dependencies

- `tree-sitter = "0.26"`, `tree-sitter-highlight = "0.26"`
- Grammar crates (0.24-0.25) → depend on `tree-sitter-language = "0.1"` (0.26-compatible)
- `once_cell = "1"` — Lazy initialization
- `zelkova-config` — `CodeTheme::from_editor_colors` references `EditorColors`
