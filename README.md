# Zelkova

A Markdown note-taking application built with [GPUI](https://github.com/zed-industries/zed) and [Tree-sitter](https://tree-sitter.github.io/tree-sitter/).

## Features

- **Inline Markdown editing** with live syntax highlighting — headings, bold, italic, strikethrough, code spans, links, images, math
- **Code block highlighting** via Tree-sitter — Rust, JavaScript/TypeScript, Python, Go, C
- **Markdown preview** side-by-side with the editor
- **Full-text search** powered by [Tantivy](https://github.com/quickwit-oss/tantivy)
- **YAML frontmatter** — title, tags, timestamps managed automatically
- **Client-server architecture** — GUI and CLI communicate with a background daemon over Unix sockets
- **Catppuccin Mocha** color scheme (configurable via TOML)

## Architecture

```
┌─────────────┐   ┌──────────────┐
│  zelkova    │   │ zelkova-cli  │
│  (GUI/GPUI) │   │  (terminal)  │
└──────┬──────┘   └──────┬───────┘
       │  JSON-RPC 2.0   │
       │  (Unix socket)  │
       └────────┬────────┘
           ┌────┴────┐
           │zelkovad │  ← background daemon
           │(indexer,│     file watcher,
           │ watcher)│     search engine
           └────┬────┘
                │ fs
           ┌────┴────┐
           │  vault  │  ← ~/Notes/
           └─────────┘
```

**10 crates** with clear separation of concerns:

| Crate | Role |
|---|---|
| `gui` | GPUI editor, preview, tabs, sidebar |
| `daemon` | Background indexing, RPC server, file watcher |
| `cli` | Terminal commands (search, list, create, tags) |
| `markdown` | Markdown parser → AST (Block/Inline enums) |
| `highlight` | Tree-sitter code block syntax highlighting |
| `rope` | B-tree text buffer with undo/redo |
| `note_core` | Note data model, vault CRUD |
| `rpc` | JSON-RPC 2.0 over Unix domain sockets |
| `search` | Full-text search (Tantivy backend) |
| `config` | TOML configuration (app, keymap, theme) |

## Getting Started

### Prerequisites

- Rust 1.85+ (edition 2024)
- Linux (GPUI requirement)
- A font that supports Japanese (optional, for CJK text editing)

### Build

```bash
git clone https://github.com/oyatomo/zelkova.git
cd zelkova
cargo build --release
```

### Run

```bash
# Start the daemon (background)
./target/release/zelkovad &

# Launch the GUI
./target/release/zelkova

# Or use the CLI
./target/release/zelkova-cli search "query"
./target/release/zelkova-cli list
./target/release/zelkova-cli create "My Note"
```

## Configuration

All configuration lives under `~/.config/zelkova/`:

| File | Purpose |
|---|---|
| `config.toml` | Vault path, daemon socket |
| `keymap.toml` | Custom key bindings with leader key |
| `theme.toml` | UI colors, editor colors, code syntax colors |

All fields have sensible defaults (Catppuccin Mocha) — create only the fields you want to override.

### Example `theme.toml`

```toml
[editor]
heading_fg = "#89b4fa"
code_bg = "#313244"
code_keyword = "#cba6f7"
code_string = "#a6e3a1"
```

## Tech Stack

- **UI Framework**: [GPUI 0.2](https://github.com/zed-industries/zed) (Zed's GPU-accelerated UI)
- **Syntax Highlighting**: [Tree-sitter](https://tree-sitter.github.io/tree-sitter/) (Rust, JS/TS, Python, Go, C)
- **Text Buffer**: Custom B-tree Rope with undo/redo
- **Search**: [Tantivy](https://github.com/quickwit-oss/tantivy) full-text search engine
- **IPC**: JSON-RPC 2.0 over Unix domain sockets
- **Serialization**: serde (TOML, YAML, JSON)

## License

All rights reserved.
