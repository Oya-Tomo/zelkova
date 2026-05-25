# Zelkova Architecture

## Overview

GPUI 0.2ベースのMarkdownノートアプリ。Unix domain socket + JSON-RPC 2.0でGUI/CLIとデーモンが通信するクライアント・サーバー構成。

```
zelkova (GUI)  ──socket──>  zelkovad (daemon)  ──fs──>  ~/Notes/
zelkova-cli    ──socket──>  zelkovad
```

## Crate Graph

```
config ──────────┐
note_core ───────┤
rope ────────────┤
markdown ────────┤
highlight ───────┤──► gui (GPUI 0.2 binary)
rpc ─────────────┘
              │
config ───────┤
note_core ────┤──► cli (clap binary)
rpc ──────────┘
              │
note_core ────┐
config ───────┤
search ───────┤──► daemon (binary)
rpc ──────────┘
              │
note_core ────┐──► search (Tantivy backend)
              │
highlight ────┘──► (config only dependency)
```

## Workspace Members (10 crates)

| Crate | Binary | Role |
|---|---|---|
| `gui` | `zelkova` | GPUI editor, preview, tabs, sidebar |
| `daemon` | `zelkovad` | Background note indexing, RPC server |
| `cli` | `zelkova-cli` | Terminal commands: search, list, create |
| `config` | — | App/keymap/theme TOML configuration |
| `note_core` | — | Note data model, vault CRUD, frontmatter |
| `markdown` | — | Markdown parser (AST: Block/Inline enums) |
| `highlight` | — | Tree-sitter code block syntax highlighting |
| `rope` | — | B-tree text buffer with undo/redo |
| `rpc` | — | JSON-RPC 2.0 over Unix sockets |
| `search` | — | Full-text search (Tantivy backend) |

## Key Design Decisions

### ResolvedColors Pattern

GUIのハイライトシステムでは、テーマ変更時に全ての色を一度だけ `parse_hex` で Hsla に変換し、`ResolvedColors` 構造体にキャッシュする。フレームごとに文字列パースを繰り返さない。

```rust
// highlight.rs
pub struct ResolvedColors {
    pub heading_fg: Hsla,  // pre-parsed
    pub code_bg: Hsla,
    code_syntax: [Hsla; 12],  // Tree-sitter 12 highlight classes
    // ...
}
```

### Deterministic Selection Overlay

GPUIの `combine_highlights` は内部で HashSet を使い反復順序が非決定的。選択背景には代わりに `overlay_selection()` を使う — ハイライト範囲を選択境界で分割し、確実に選択背景が優先される。

### Lazy Highlight Rendering

ハイライト計算は重いため、初回フレームはプレーンテキストで表示し、`highlights_dirty` フラグで次フレームに計算を遅延させる。

### File Watching

Daemonは2秒間隔のポーリングでファイル変更を検知し、自動的に検索インデックスを更新する。

## Configuration

- **App config**: `~/.config/zelkova/config.toml` (vault path, daemon socket)
- **Keymap**: `~/.config/zelkova/keymap.toml` (leader key, custom bindings)
- **Theme**: `~/.config/zelkova/theme.toml` (UI colors, editor colors, 12 code syntax colors)

全設定項目はserde defaultで補完されるため、部分的なTOMLファイルで動作する。
