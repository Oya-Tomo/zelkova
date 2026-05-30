# note_core

## Role

A crate that manages the Markdown note data model (Frontmatter + body) and the filesystem-based Vault (note collection).

## Module Layout

```
src/
├── lib.rs     Re-exports of Frontmatter, Note
├── note.rs    Frontmatter, Note structs, accessors
└── vault.rs   Vault struct, CRUD operations, file I/O
```

## Dependencies

- `chrono` — `DateTime<Utc>` (creation and update timestamps)
- `serde` / `serde_yaml` — YAML frontmatter serialization/deserialization
- `uuid` — `Uuid` (note ID)
- `anyhow` — Error handling

## Key Types / APIs

### Frontmatter (note.rs)

Note metadata. Serialized as YAML frontmatter.

```rust
struct Frontmatter {
    id: Uuid,                    // Unique identifier
    title: String,               // Title
    tags: HashSet<String>,       // Tags (default: empty set)
    created: DateTime<Utc>,      // Creation timestamp
    updated: DateTime<Utc>,      // Update timestamp
}
```

### Note (note.rs)

Frontmatter + body + file path.

```rust
struct Note {
    frontmatter: Frontmatter,
    content: String,     // Body text after frontmatter
    path: PathBuf,       // Absolute path on the filesystem
}
```

**Accessor methods:** `id()`, `title()`, `tags()`, `created()`, `updated()`

### Vault (vault.rs)

A struct that manages a collection of notes on the filesystem.

```rust
struct Vault {
    vault_path: PathBuf,  // Root directory of the Vault
}
```

**CRUD methods:**

| Method | Signature | Description |
|---|---|---|
| `new(vault_path)` | `PathBuf -> Result<Self>` | Create directory and initialize Vault |
| `list_notes()` | `&self -> Result<Vec<Note>>` | Recursively collect all notes |
| `get_note(relative_path)` | `&self, &Path -> Result<Option<Note>>` | Get note by relative path |
| `create_note(title, parent_dir, tags)` | `&self, &str, Option<&Path>, HashSet<String> -> Result<Note>` | Create a new note |
| `delete_note(relative_path)` | `&self, &Path -> Result<()>` | Delete a note |
| `all_tags()` | `&self -> Result<HashSet<String>>` | Collect tags from all notes |

### File Format

Notes use YAML frontmatter + Markdown body format:

```markdown
---
id: "550e8400-e29b-41d4-a716-446655440000"
title: "Note Title"
tags:
  - rust
  - note
created: 2025-01-15T10:30:00Z
updated: 2025-01-15T10:30:00Z
---
Note body text
```

### Internal Functions

| Function | Description |
|---|---|
| `sanitize_filename(title)` | Replace `/` with `-`, remove NULL bytes, truncate to 128 characters |
| `parse_frontmatter(content)` | Parse YAML delimited by `---` → `(Frontmatter, body)` |
| `format_note_file(frontmatter, body)` | Serialize Frontmatter + body into file format |

### Directory Traversal (collect_notes)

Recursively scans under `vault_path`:
- Hidden directories (starting with `.`) are skipped
- Only `.md` extension files are collected
- Parse failures print a warning to stderr and continue

## Data Flow

```
create_note("Title", Some(Path("sub")), {"rust"})
    │
    ├── Uuid::new_v4() → Generate ID
    ├── Utc::now() → Generate timestamp
    ├── sanitize_filename("Title") → "Title"
    ├── Write to vault_path/sub/Title.md
    │     Content:
    │     ---
    │     id: "..."
    │     title: "Title"
    │     tags:
    │       - rust
    │     created: ...
    │     updated: ...
    │     ---
    │
    └── Return Note { frontmatter, content: "", path }


list_notes()
    │
    ├── collect_notes(vault_path)
    │     ├── Scan via read_dir()
    │     ├── Skip directories starting with .
    │     ├── Target only .md files
    │     └── parse_note_file() → Note
    │
    └── Return Vec<Note>
```
