# config

## Role

Configuration management crate that loads application settings (notes, daemon, keymap, theme) from TOML files.

## Module Layout

```
src/
├── lib.rs      AppConfig, NoteConfig, DaemonConfig, McpConfig, file I/O
├── keymap.rs   KeymapConfig, BindingConfig, leader key resolution
└── theme.rs    ThemeConfig, UiColors, EditorColors (25+ color fields), HEX parser
```

## Dependencies

- `serde` / `toml` — TOML serialization/deserialization
- `dirs` — XDG configuration directory resolution
- `anyhow` — Error handling

## Key Types / APIs

### AppConfig (lib.rs)

Root struct for application settings. Loaded from `~/.config/zelkova/config.toml`.

```rust
struct AppConfig {
    note: NoteConfig,    // Note settings
    daemon: DaemonConfig, // Daemon settings
    mcp: McpConfig,      // MCP settings
}
```

**Methods:**
- `load()` — Reads `config.toml`; returns defaults if the file does not exist
- `config_path()` — Returns `~/.config/zelkova/config.toml`

### NoteConfig

```rust
struct NoteConfig {
    vault_path: PathBuf,        // Default: ~/Notes
    default_extension: String,  // Default: "md"
}
```

### DaemonConfig

```rust
struct DaemonConfig {
    socket_path: PathBuf,    // Default: /tmp/zelkova.sock
    index_on_start: bool,    // Default: true
}
```

### McpConfig

```rust
struct McpConfig {
    enabled: bool,  // Default: true
}
```

### KeymapConfig (keymap.rs)

Keyboard shortcut settings. Loaded from `~/.config/zelkova/keymap.toml`.

```rust
struct KeymapConfig {
    leader: String,               // Default: "space"
    bindings: Vec<BindingConfig>,  // Key binding list
}

struct BindingConfig {
    key: String,               // e.g. "ctrl-p"
    action: String,            // e.g. "open_command_palette"
    context: Option<String>,   // Context restriction (unused field)
}
```

**Default bindings:**

| Key | Action |
|---|---|
| `ctrl-p` | `open_command_palette` |
| `ctrl-shift-f` | `search_notes` |
| `ctrl-n` | `create_note` |
| `ctrl-s` | `save_note` |
| `ctrl-b` | `toggle_sidebar` |
| `ctrl-q` | `quit` |

**Methods:**
- `load()` — Reads `keymap.toml`; returns defaults if the file does not exist
- `resolved_bindings()` — Replaces `"leader"` strings in bindings with the actual leader key

### ThemeConfig (theme.rs)

UI and editor color theme settings. Loaded from `~/.config/zelkova/theme.toml`. Defaults are based on Catppuccin Mocha.

```rust
struct ThemeConfig {
    ui: UiColors,
    editor: EditorColors,
}
```

### UiColors (5 fields)

| Field | Default | Purpose |
|---|---|---|
| `bg` | `#1e1e2e` | Main background |
| `sidebar_bg` | `#181825` | Sidebar background |
| `border` | `#313244` | Border |
| `text` | `#cdd6f4` | Main text |
| `text_dim` | `#a6adc8` | Secondary text |

### EditorColors (27 fields)

**Markdown colors:**

| Field | Default | Purpose |
|---|---|---|
| `heading_fg` | `#89b4fa` | Heading text |
| `heading_marker` | `#89b4fa` | `#` marker |
| `list_marker` | `#f9e2af` | List marker |
| `code_bg` | `#313244` | Code block background |
| `code_fg` | `#a6e3a1` | Code block text |
| `link_fg` | `#89b4fa` | Link |
| `image_marker` | `#7f849c` | Image marker |
| `quote_fg` | `#9399b2` | Blockquote text |
| `quote_border` | `#585b70` | Blockquote border |
| `math_fg` | `#cba6f7` | Math expression |
| `strikethrough_fg` | `#7f849c` | Strikethrough |
| `bold_fg` | `#f9e2af` | Bold text |
| `italic_fg` | `#f5c2e7` | Italic text |
| `bold_weight` | `700` | Bold weight |
| `text_dim` | `#a6adc8` | Secondary text |

**Syntax highlighting colors (12 fields):**

| Field | Default | Tree-sitter class |
|---|---|---|
| `code_keyword` | `#cba6f7` | keyword |
| `code_function` | `#89b4fa` | function |
| `code_string` | `#a6e3a1` | string |
| `code_number` | `#fab387` | number |
| `code_comment` | `#6c7086` | comment |
| `code_type` | `#f9e2af` | type |
| `code_constant` | `#fab387` | constant |
| `code_operator` | `#89dceb` | operator |
| `code_property` | `#89b4fa` | property |
| `code_tag` | `#f38ba8` | tag |
| `code_punctuation` | `#6c7086` | punctuation |
| `code_attribute` | `#f9e2af` | attribute |

**Helper methods:**
- `EditorColors::parse_hex(hex)` — Parses `"#RRGGBB"` into `(u8, u8, u8)`
- `UiColors::parse_hex(hex)` — Delegates to `EditorColors::parse_hex` internally

## Data Flow

```
~/.config/zelkova/
├── config.toml     → AppConfig::load()    → NoteConfig, DaemonConfig, McpConfig
├── keymap.toml     → KeymapConfig::load() → BindingConfig[]
└── theme.toml      → ThemeConfig::load()  → UiColors, EditorColors

Each load():
  File exists? ──No──> Return default values
       │
      Yes
       │
  Read file → toml::from_str() → Result<T>
```

### Partial TOML support

All structs use `#[serde(default)]`. Unspecified fields are filled with default values, so users only need to write the settings they want to change:

```toml
# Example config.toml (note section only)
[note]
vault_path = "/tmp/test-vault"
# daemon, mcp sections can be omitted — defaults will be used
```

## Configuration File Paths

| File | Path | Corresponding struct |
|---|---|---|
| `config.toml` | `~/.config/zelkova/config.toml` | `AppConfig` |
| `keymap.toml` | `~/.config/zelkova/keymap.toml` | `KeymapConfig` |
| `theme.toml` | `~/.config/zelkova/theme.toml` | `ThemeConfig` |
