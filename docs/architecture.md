# Zelkova Architecture

## Overview

A Markdown note-taking application built on GPUI 0.2. Client-server architecture where the GUI and CLI communicate with the daemon via Unix domain sockets and JSON-RPC 2.0.

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

In the GUI highlight system, all colors are converted from hex to Hsla via `parse_hex` only once when the theme changes, then cached in the `ResolvedColors` struct. This avoids repeated string parsing per frame.

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

GPUI's `combine_highlights` uses a HashSet internally, resulting in non-deterministic iteration order. For selection backgrounds, `overlay_selection()` is used instead — it splits highlight ranges at selection boundaries, ensuring the selection background always takes priority.

### Lazy Highlight Rendering

Highlight computation is expensive, so the first frame renders plain text. The `highlights_dirty` flag defers computation to the next frame.

### File Watching

The daemon detects file changes via polling at 2-second intervals and automatically updates the search index.

## Configuration

- **App config**: `~/.config/zelkova/config.toml` (vault path, daemon socket, theme)
- **Keymap**: `~/.config/zelkova/keymap.toml` (leader key, custom bindings)

All configuration fields are filled in with serde defaults, so partial TOML files work correctly.
