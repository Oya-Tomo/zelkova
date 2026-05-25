# config

## Role

アプリケーション全体の設定（ノート、デーモン、キーマップ、テーマ）をTOMLファイルから読み込む設定管理crate。

## Module Layout

```
src/
├── lib.rs      AppConfig, NoteConfig, DaemonConfig, McpConfig, ファイルI/O
├── keymap.rs   KeymapConfig, BindingConfig, リーダーキー解決
└── theme.rs    ThemeConfig, UiColors, EditorColors (25+色フィールド), HEXパーサー
```

## Dependencies

- `serde` / `toml` — TOMLのシリアライズ・デシリアライズ
- `dirs` — XDG設定ディレクトリの解決
- `anyhow` — エラーハンドリング

## Key Types / APIs

### AppConfig (lib.rs)

アプリケーション設定のルート構造体。`~/.config/zelkova/config.toml`から読み込む。

```rust
struct AppConfig {
    note: NoteConfig,    // ノート関連設定
    daemon: DaemonConfig, // デーモン関連設定
    mcp: McpConfig,      // MCP関連設定
}
```

**メソッド:**
- `load()` — `config.toml`を読み込み、存在しなければデフォルトを返す
- `config_path()` — `~/.config/zelkova/config.toml`を返す

### NoteConfig

```rust
struct NoteConfig {
    vault_path: PathBuf,        // デフォルト: ~/Notes
    default_extension: String,  // デフォルト: "md"
}
```

### DaemonConfig

```rust
struct DaemonConfig {
    socket_path: PathBuf,    // デフォルト: /tmp/zelkova.sock
    index_on_start: bool,    // デフォルト: true
}
```

### McpConfig

```rust
struct McpConfig {
    enabled: bool,  // デフォルト: true
}
```

### KeymapConfig (keymap.rs)

キーボードショートカット設定。`~/.config/zelkova/keymap.toml`から読み込む。

```rust
struct KeymapConfig {
    leader: String,               // デフォルト: "space"
    bindings: Vec<BindingConfig>,  // キーバインド一覧
}

struct BindingConfig {
    key: String,               // 例: "ctrl-p"
    action: String,            // 例: "open_command_palette"
    context: Option<String>,   // コンテキスト制限 (未使用フィールド)
}
```

**デフォルトバインド:**

| キー | アクション |
|---|---|
| `ctrl-p` | `open_command_palette` |
| `ctrl-shift-f` | `search_notes` |
| `ctrl-n` | `create_note` |
| `ctrl-s` | `save_note` |
| `ctrl-b` | `toggle_sidebar` |
| `ctrl-q` | `quit` |

**メソッド:**
- `load()` — `keymap.toml`を読み込み、存在しなければデフォルトを返す
- `resolved_bindings()` — バインド内の`"leader"`文字列を実際のリーダーキーに置換

### ThemeConfig (theme.rs)

UI・エディタのカラーテーマ設定。`~/.config/zelkova/theme.toml`から読み込む。デフォルトはCatppuccin Mochaベース。

```rust
struct ThemeConfig {
    ui: UiColors,
    editor: EditorColors,
}
```

### UiColors (5フィールド)

| フィールド | デフォルト | 用途 |
|---|---|---|
| `bg` | `#1e1e2e` | メイン背景 |
| `sidebar_bg` | `#181825` | サイドバー背景 |
| `border` | `#313244` | ボーダー |
| `text` | `#cdd6f4` | メインテキスト |
| `text_dim` | `#a6adc8` | 補助テキスト |

### EditorColors (27フィールド)

**Markdown用色:**

| フィールド | デフォルト | 用途 |
|---|---|---|
| `heading_fg` | `#89b4fa` | 見出しテキスト |
| `heading_marker` | `#89b4fa` | `#` マーカー |
| `list_marker` | `#f9e2af` | リストマーカー |
| `code_bg` | `#313244` | コードブロック背景 |
| `code_fg` | `#a6e3a1` | コードブロックテキスト |
| `link_fg` | `#89b4fa` | リンク |
| `image_marker` | `#7f849c` | 画像マーカー |
| `quote_fg` | `#9399b2` | 引用テキスト |
| `quote_border` | `#585b70` | 引用ボーダー |
| `math_fg` | `#cba6f7` | 数式 |
| `strikethrough_fg` | `#7f849c` | 取り消し線 |
| `bold_fg` | `#f9e2af` | 太字 |
| `italic_fg` | `#f5c2e7` | イタリック |
| `bold_weight` | `700` | 太字ウェイト |
| `text_dim` | `#a6adc8` | 補助テキスト |

**シンタックスハイライト用色 (12フィールド):**

| フィールド | デフォルト | 対応Tree-sitterクラス |
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

**ヘルパーメソッド:**
- `EditorColors::parse_hex(hex)` — `"#RRGGBB"` を `(u8, u8, u8)` にパース
- `UiColors::parse_hex(hex)` — 内部で`EditorColors::parse_hex`に委譲

## Data Flow

```
~/.config/zelkova/
├── config.toml     → AppConfig::load()    → NoteConfig, DaemonConfig, McpConfig
├── keymap.toml     → KeymapConfig::load() → BindingConfig[]
└── theme.toml      → ThemeConfig::load()  → UiColors, EditorColors

各load():
  ファイル存在? ──No──> デフォルト値を返す
       │
      Yes
       │
  ファイル読み込み → toml::from_str() → Result<T>
```

### 部分TOML対応

全構造体で`#[serde(default)]`を使用。未指定フィールドはデフォルト値で補完されるため、ユーザーは変更したい項目のみを記述可能:

```toml
# config.tomlの例 (noteセクションのみ)
[note]
vault_path = "/tmp/test-vault"
# daemon, mcpセクションは省略可 → デフォルト値が使用される
```

## 設定ファイルパス一覧

| ファイル | パス | 対応構造体 |
|---|---|---|
| `config.toml` | `~/.config/zelkova/config.toml` | `AppConfig` |
| `keymap.toml` | `~/.config/zelkova/keymap.toml` | `KeymapConfig` |
| `theme.toml` | `~/.config/zelkova/theme.toml` | `ThemeConfig` |
