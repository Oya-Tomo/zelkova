# search

## Role

A crate that abstracts full-text search indexing for notes and provides a Tantivy-based backend implementation.

## Module Layout

```
src/
├── lib.rs               Module declarations, default_search_index factory function
├── engine/
│   └── mod.rs           SearchDocument, SearchQuery, SearchResult, SearchIndex trait
└── tantivy_backend.rs   TantivyIndex (enabled by feature "tantivy")
```

## Dependencies

- `uuid` — Document ID
- `anyhow` — Error handling
- `tantivy` (feature-gated) — Full-text search engine

## Key Types / APIs

### SearchDocument (engine/mod.rs)

A document to be indexed.

```rust
struct SearchDocument {
    id: Uuid,
    title: String,
    content: String,
    tags: Vec<String>,
    path: PathBuf,
}
```

### SearchQuery (engine/mod.rs)

A search query.

```rust
struct SearchQuery {
    text: String,           // Full-text search query
    limit: Option<usize>,   // Maximum number of results (default: 20)
    tags: Vec<String>,      // Tag filter (AND condition)
}
```

### SearchResult (engine/mod.rs)

A single search result.

```rust
struct SearchResult {
    id: Uuid,
    title: String,
    path: PathBuf,
    score: f32,         // Relevance score
    snippet: String,    // Snippet (currently unimplemented: empty string)
}
```

### SearchIndex trait (engine/mod.rs)

Abstract interface for search backends.

```rust
trait SearchIndex: Send + Sync {
    fn add_document(&self, doc: &SearchDocument) -> Result<()>;
    fn remove_document(&self, id: &Uuid) -> Result<()>;
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;
    fn rebuild(&self, docs: &[SearchDocument]) -> Result<()>;
}
```

### default_search_index (lib.rs)

```rust
fn default_search_index(path: &Path) -> Result<Box<dyn SearchIndex>>
```

Selects the backend based on feature flags:
- `"tantivy"` feature enabled → returns `TantivyIndex::open(path)`
- Feature disabled → compilation error

### TantivyIndex (tantivy_backend.rs)

Tantivy-based SearchIndex implementation. Enabled by feature `"tantivy"`.

```rust
struct TantivyIndex {
    index: Index,
    schema: Schema,
    writer: Mutex<IndexWriter>,
}
```

**Index schema (5 fields):**

| Field | Type | Options | Purpose |
|---|---|---|---|
| `id` | STRING | STORED | Unique document identifier |
| `title` | TEXT | STORED | Title (tokenized) |
| `content` | TEXT | STORED | Body text (tokenized) |
| `tags` | TEXT | STORED | Tags (joined with spaces before storing) |
| `path` | TEXT | STORED | File path |

**SearchIndex implementation:**

| Method | Implementation details |
|---|---|
| `add_document` | Build Tantivy document via `doc!` macro, add to writer → commit |
| `remove_document` | Delete by id field Term → commit |
| `search` | QueryParser (targeting title, content, tags) + tag TermQuery → BooleanQuery → TopDocs |
| `rebuild` | `delete_all_documents()` → commit → add_document for all documents |

**Search query construction:**
1. `text` is non-empty → parse via QueryParser → add as `Occur::Must`
2. Each `tag` → TermQuery on tags field → add as `Occur::Must`
3. Combine all conditions via BooleanQuery (AND condition)
4. No conditions → wildcard query (`*`)

**Thread safety:** `IndexWriter` is protected by `Mutex`. Satisfies `SearchIndex: Send + Sync`.

**Index persistence:** Saves the Tantivy index to the specified directory. Determines whether to create new or open existing based on the presence of `meta.json`.

## Data Flow

```
Adding a note:
  SearchDocument { id, title, content, tags, path }
      │
      TantivyIndex::add_document()
      │
      ├── Map fields via doc! macro
      │     tags: Vec<String> → single space-separated text
      │
      ├── writer.add_document(doc)
      └── writer.commit()

Searching:
  SearchQuery { text: "Rust", tags: ["programming"], limit: Some(10) }
      │
      TantivyIndex::search()
      │
      ├── QueryParser: search "Rust" in title, content, tags fields
      ├── TermQuery: search "programming" in tags field
      ├── BooleanQuery: Must (AND) combination
      ├── Retrieve via TopDocs::with_limit(10)
      └── Convert to SearchResult[] (id, title, path, score)

Index rebuild:
  rebuild(docs)
      │
      ├── delete_all_documents() → commit
      └── for each doc: add_document(doc)
```
