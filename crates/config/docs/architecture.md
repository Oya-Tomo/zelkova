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

### UiConfig (lib.rs — `[ui]` section in config.toml)

Theme settings loaded from the `[ui]` section of `~/.config/zelkova/config.toml`. Defaults are Catppuccin Mocha.

```rust
struct UiConfig {
    theme: String,          // bundled theme name (e.g. "catppuccin")
    mode: String,           // "dark" or "light"
    override_path: Option<String>,  // optional override JSON
}
```

Colors are resolved at runtime from bundled theme JSONs (see `crates/gui/themes/`).
The `override_path` can point to a user-provided JSON that merges on top of the base theme.
Markdown rendering colors come from the `"markdown"` section of each theme JSON, with
automatic fallbacks derived from syntax and UI colors when absent.

## Data Flow

```
~/.config/zelkova/
├── config.toml     → AppConfig::load()    → NoteConfig, DaemonConfig, McpConfig, UiConfig
├── keymap.toml     → KeymapConfig::load() → BindingConfig[]
└── themes/         → User-provided override JSONs (optional)

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
| `config.toml` | `~/.config/zelkova/config.toml` | `AppConfig` (includes `UiConfig`) |
| `keymap.toml` | `~/.config/zelkova/keymap.toml` | `KeymapConfig` |
