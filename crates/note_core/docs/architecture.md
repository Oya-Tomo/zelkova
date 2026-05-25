# note_core

## Role

Markdownノートのデータモデル (Frontmatter + 本文) と、ファイルシステム上のVault (ノート集) を管理するcrate。

## Module Layout

```
src/
├── lib.rs     Frontmatter, Noteのre-export
├── note.rs    Frontmatter, Note構造体、アクセサ
└── vault.rs   Vault構造体、CRUD操作、ファイルI/O
```

## Dependencies

- `chrono` — `DateTime<Utc>` (作成日時・更新日時)
- `serde` / `serde_yaml` — YAML frontmatterのシリアライズ・デシリアライズ
- `uuid` — `Uuid` (ノートID)
- `anyhow` — エラーハンドリング

## Key Types / APIs

### Frontmatter (note.rs)

ノートのメタデータ。YAML frontmatterとしてシリアライズされる。

```rust
struct Frontmatter {
    id: Uuid,                    // 一意識別子
    title: String,               // タイトル
    tags: HashSet<String>,       // タグ (デフォルト空集合)
    created: DateTime<Utc>,      // 作成日時
    updated: DateTime<Utc>,      // 更新日時
}
```

### Note (note.rs)

Frontmatter + 本文 + ファイルパス。

```rust
struct Note {
    frontmatter: Frontmatter,
    content: String,     // frontmatter以降の本文
    path: PathBuf,       // ファイルシステム上の絶対パス
}
```

**アクセサメソッド:** `id()`, `title()`, `tags()`, `created()`, `updated()`

### Vault (vault.rs)

ファイルシステム上のノート集を管理する構造体。

```rust
struct Vault {
    vault_path: PathBuf,  // Vaultのルートディレクトリ
}
```

**CRUDメソッド:**

| メソッド | シグネチャ | 説明 |
|---|---|---|
| `new(vault_path)` | `PathBuf -> Result<Self>` | ディレクトリを作成しVaultを初期化 |
| `list_notes()` | `&self -> Result<Vec<Note>>` | 全ノートを再帰的に収集 |
| `get_note(relative_path)` | `&self, &Path -> Result<Option<Note>>` | 相対パスでノートを取得 |
| `create_note(title, parent_dir, tags)` | `&self, &str, Option<&Path>, HashSet<String> -> Result<Note>` | 新規ノートを作成 |
| `delete_note(relative_path)` | `&self, &Path -> Result<()>` | ノートを削除 |
| `all_tags()` | `&self -> Result<HashSet<String>>` | 全ノートからタグを収集 |

### ファイル形式

ノートはYAML frontmatter + Markdown本文の形式:

```markdown
---
id: "550e8400-e29b-41d4-a716-446655440000"
title: "ノートタイトル"
tags:
  - rust
  - note
created: 2025-01-15T10:30:00Z
updated: 2025-01-15T10:30:00Z
---
ノートの本文
```

### 内部関数

| 関数 | 説明 |
|---|---|
| `sanitize_filename(title)` | `/`を`-`に置換、NULL除去、128文字で切り詰め |
| `parse_frontmatter(content)` | `---`区切りのYAMLをパース → `(Frontmatter, body)` |
| `format_note_file(frontmatter, body)` | Frontmatter + 本文をファイル形式にシリアライズ |

### ディレクトリ巡回 (collect_notes)

`vault_path`以下を再帰的に走査:
- 隠しディレクトリ (`.`で始まる) はスキップ
- `.md`拡張子のファイルのみ収集
- パース失敗時は警告をstderrに出力し続行

## Data Flow

```
create_note("Title", Some(Path("sub")), {"rust"})
    │
    ├── Uuid::new_v4() → ID生成
    ├── Utc::now() → タイムスタンプ生成
    ├── sanitize_filename("Title") → "Title"
    ├── vault_path/sub/Title.md に書き込み
    │     内容:
    │     ---
    │     id: "..."
    │     title: "Title"
    │     tags:
    │       - rust
    │     created: ...
    │     updated: ...
    │     ---
    │
    └── Note { frontmatter, content: "", path } を返す


list_notes()
    │
    ├── collect_notes(vault_path)
    │     ├── read_dir() で走査
    │     ├── .で始まるディレクトリをスキップ
    │     ├── .mdファイルのみ対象
    │     └── parse_note_file() → Note
    │
    └── Vec<Note> を返す
```
