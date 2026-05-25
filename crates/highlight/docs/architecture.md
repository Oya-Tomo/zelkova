# highlight

## Role

Tree-sitterベースのコードブロックシンタックスハイライト。GUIエディタのMarkdownコードフェンス内のソースコードを、言語ごとに構文解析してスタイル付き範囲 (`StyledRange`) を返す。

このクレートはGPUIに依存しない。色の解決（文字列→Hsla変換）はGUI側の `ResolvedColors` が行う。

## Module Layout

```
src/
├── lib.rs     言語設定の遅延初期化、resolve_language、highlight_code
└── theme.rs   CodeTheme、HIGHLIGHT_NAMES、色マッピング定義
```

## Supported Languages

| Language | Grammar Crate | Config Key | Query Constant |
|---|---|---|---|
| Rust | tree-sitter-rust 0.24 | `"rust"` | `HIGHLIGHTS_QUERY` |
| JS/TS | tree-sitter-javascript 0.25 | `"javascript"` | `HIGHLIGHT_QUERY` |
| Python | tree-sitter-python 0.25 | `"python"` | `HIGHLIGHTS_QUERY` |
| Go | tree-sitter-go 0.25 | `"go"` | `HIGHLIGHTS_QUERY` |
| C | tree-sitter-c 0.24 | `"c"` | `HIGHLIGHT_QUERY` |

注意: grammar crateのバージョンはバラバラだが、全て `tree-sitter-language = "0.1"` に依存するため `tree-sitter = "0.26"` と互換。

## Key APIs

### `resolve_language(info: &str) -> Option<&'static str>`

Markdownフェンスの情報文字列を内部言語キーに変換。

```
"rust" | "rs"           → "rust"
"javascript" | "js"     → "javascript"
"typescript" | "ts"     → "javascript"  // TSはJS文法でハイライト
"python" | "py"         → "python"
"go" | "golang"         → "go"
"c" | "h"               → "c"
```

### `highlight_code(code: &str, language: &str) -> Vec<StyledRange>`

コード文字列をTree-sitterで解析し、`StyledRange` のリストを返す。各 `StyledRange` はバイト範囲とハイライトインデックスを持つ。未知の言語・空文字列は空Vec。

### `StyledRange`

```rust
pub struct StyledRange {
    pub range: Range<usize>,       // バイトオフセット範囲
    pub highlight_index: usize,    // HIGHLIGHT_NAMES のインデックス
}
```

### `HIGHLIGHT_NAMES`

Tree-sitterに渡す12個のハイライトクラス名。インデックスが `Highlight(usize)` に対応。

```rust
["attribute", "comment", "constant", "function", "keyword",
 "number", "operator", "property", "punctuation", "string", "tag", "type"]
```

### `CodeTheme`

12クラスに対応する色文字列 (`#RRGGBB`) を保持。GUI側の `ResolvedColors` が同等の機能をHslaで提供するため、この構造体は直接のレンダリングには使われない。テストとcrateの独立性維持のために存在。

- `CodeTheme::from_editor_colors(&EditorColors)` — 設定から構築
- `color_by_index(usize) -> Option<&str>` — インデックスから色文字列を逆引き

## Internal Architecture

### Lazy Configuration (`CONFIGS`)

`HighlightConfiguration` の初期化は重いため、`Lazy<HashMap>` で遅延評価。各grammarのクエリ定数名に注意:

- Rust, Python, Go: `HIGHLIGHTS_QUERY` (複数形)
- JavaScript, C: `HIGHLIGHT_QUERY` (単数形)

### Data Flow

```
GUI: build_highlights() がフェンス言語を検出
  │
  ├─ resolve_language("rust") → Some("rust")
  │
  ├─ highlight_code(source, "rust")
  │   ├─ CONFIGS.get("rust") → &HighlightConfiguration
  │   ├─ TsHighlighter::highlight() → HighlightEventストリーム
  │   └─ イベントを畳み込んで Vec<StyledRange>
  │
  └─ GUI: ResolvedColors.syntax_color(index) → Hsla を直接適用
```

## Dependencies

- `tree-sitter = "0.26"`, `tree-sitter-highlight = "0.26"`
- Grammar crates (0.24-0.25) → 依存先は `tree-sitter-language = "0.1"` (0.26互換)
- `once_cell = "1"` — Lazy初期化
- `zelkova-config` — `CodeTheme::from_editor_colors` で `EditorColors` を参照
